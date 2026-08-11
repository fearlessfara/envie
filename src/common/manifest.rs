//! What Envie remembers about a deployed environment.
//!
//! Destroying an environment has to reproduce the wiring it was deployed with.
//! An ephemeral environment whose `api` was pointed at production's `network`
//! cannot be torn down against its own empty network state — Terraform would
//! fail to evaluate the resources it is trying to remove. So each deploy records
//! which units it touched and which environment each of their dependencies was
//! read from, and teardown replays that instead of guessing.
//!
//! The record lives next to the state it describes, in the same backend, so it
//! is shared by everyone who can deploy the environment rather than being stuck
//! on the machine that happened to run the deploy.

use crate::common::deployment::Plan;
use crate::common::environment::ResolvedEnvironment;
use crate::common::{EnvieError, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;

const MANIFEST_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentManifest {
    pub version: u32,
    pub project: String,
    pub environment: String,
    pub workspace: String,
    /// Last time a deploy updated this record, as an RFC 3339 timestamp.
    pub updated_at: String,
    /// Unit name -> what was deployed. Keyed by name because that is what both
    /// dependencies and the CLI refer to units by.
    pub units: BTreeMap<String, DeployedUnit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployedUnit {
    /// Path relative to the project root.
    pub path: String,
    /// Where this unit's state was written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_key: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<DeployedDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployedDependency {
    /// The unit that was read.
    pub unit: String,
    /// The environment reference it was read from, e.g. `stable.production`.
    pub environment: String,
}

impl EnvironmentManifest {
    /// Describe what a plan just deployed.
    pub fn from_plan(plan: &Plan) -> Self {
        let mut units = BTreeMap::new();
        for unit in &plan.units {
            units.insert(
                unit.name.clone(),
                DeployedUnit {
                    path: unit.path.to_string_lossy().replace('\\', "/"),
                    state_key: unit.target.state_path().map(str::to_string),
                    dependencies: unit
                        .dependencies
                        .iter()
                        .map(|dependency| DeployedDependency {
                            unit: dependency.unit_name.clone(),
                            environment: dependency.environment_reference.clone(),
                        })
                        .collect(),
                },
            );
        }

        Self {
            version: MANIFEST_VERSION,
            project: plan.project_name.clone(),
            environment: plan.environment.name.clone(),
            workspace: plan.environment.workspace.clone(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            units,
        }
    }

    /// Fold a newer record into this one.
    ///
    /// Deploying a single unit must not make Envie forget the rest of the
    /// environment, so units absent from `newer` are kept as they were.
    pub fn merge(&mut self, newer: EnvironmentManifest) {
        self.version = newer.version;
        self.workspace = newer.workspace;
        self.updated_at = newer.updated_at;
        for (name, unit) in newer.units {
            self.units.insert(name, unit);
        }
    }

    /// The `-E unit:environment` overrides that reproduce this deployment.
    ///
    /// Dependencies read from the environment being torn down need no override,
    /// and recording them would only add noise.
    pub fn dependency_overrides(
        &self,
        environment: &ResolvedEnvironment,
    ) -> HashMap<String, String> {
        let own = environment.reference();
        let mut overrides = HashMap::new();
        for unit in self.units.values() {
            for dependency in &unit.dependencies {
                if dependency.environment != own {
                    overrides.insert(dependency.unit.clone(), dependency.environment.clone());
                }
            }
        }
        overrides
    }

    pub fn unit_names(&self) -> Vec<String> {
        self.units.keys().cloned().collect()
    }
}

/// Where an environment's record is kept.
enum Location {
    S3 {
        bucket: String,
        key: String,
        region: Option<String>,
    },
    File(PathBuf),
}

fn location(root: &Path, environment: &ResolvedEnvironment) -> Location {
    let name = format!("{}.json", environment.name);

    if environment.backend.backend_type == "s3" {
        if let Some(bucket) = environment.backend.config.get("bucket") {
            return Location::S3 {
                bucket: bucket.clone(),
                key: format!("envie/manifests/{}", name),
                region: environment.backend.config.get("region").cloned(),
            };
        }
    }

    Location::File(root.join(".envie").join("manifests").join(name))
}

/// Record a deployment, keeping anything an earlier deploy recorded.
pub fn save(
    root: &Path,
    environment: &ResolvedEnvironment,
    manifest: EnvironmentManifest,
) -> Result<()> {
    let merged = match load(root, environment)? {
        Some(mut existing) => {
            existing.merge(manifest);
            existing
        }
        None => manifest,
    };

    let body = serde_json::to_string_pretty(&merged)
        .map_err(|e| EnvieError::ConfigError(format!("could not serialise the manifest: {}", e)))?;

    match location(root, environment) {
        Location::S3 {
            bucket,
            key,
            region,
        } => {
            let temporary = std::env::temp_dir().join(format!(
                "envie-manifest-{}-{}.json",
                environment.name,
                std::process::id()
            ));
            std::fs::write(&temporary, &body)?;

            let mut command = Command::new("aws");
            command.args([
                "s3api",
                "put-object",
                "--bucket",
                &bucket,
                "--key",
                &key,
                "--body",
                &temporary.to_string_lossy(),
                "--content-type",
                "application/json",
            ]);
            if let Some(region) = &region {
                command.args(["--region", region]);
            }

            let output = command.output();
            let _ = std::fs::remove_file(&temporary);

            let output = output?;
            if !output.status.success() {
                // Losing the record is not worth failing a successful deploy
                // over; teardown can still be pointed by hand with -E.
                return Err(EnvieError::ProcessError(format!(
                    "could not record the deployment at s3://{}/{}: {}",
                    bucket,
                    key,
                    String::from_utf8_lossy(&output.stderr).trim()
                )));
            }
        }
        Location::File(path) => {
            std::fs::create_dir_all(path.parent().unwrap())?;
            std::fs::write(path, body)?;
        }
    }

    Ok(())
}

/// What Envie recorded for this environment, if anything.
///
/// A missing record is normal: the environment may predate this version of
/// Envie, or may never have been deployed.
pub fn load(root: &Path, environment: &ResolvedEnvironment) -> Result<Option<EnvironmentManifest>> {
    let body = match location(root, environment) {
        Location::S3 {
            bucket,
            key,
            region,
        } => {
            let destination = std::env::temp_dir().join(format!(
                "envie-manifest-read-{}-{}.json",
                environment.name,
                std::process::id()
            ));

            let mut command = Command::new("aws");
            command.args([
                "s3api",
                "get-object",
                "--bucket",
                &bucket,
                "--key",
                &key,
                &destination.to_string_lossy(),
            ]);
            if let Some(region) = &region {
                command.args(["--region", region]);
            }

            let output = command.output()?;
            if !output.status.success() {
                let _ = std::fs::remove_file(&destination);
                return Ok(None);
            }

            let body = std::fs::read_to_string(&destination)?;
            let _ = std::fs::remove_file(&destination);
            body
        }
        Location::File(path) => {
            if !path.exists() {
                return Ok(None);
            }
            std::fs::read_to_string(path)?
        }
    };

    // A record Envie cannot read is treated as absent rather than fatal: it must
    // never be the reason an environment cannot be destroyed.
    Ok(serde_json::from_str(&body).ok())
}

/// Forget an environment, once it no longer exists.
pub fn remove(root: &Path, environment: &ResolvedEnvironment) -> Result<()> {
    match location(root, environment) {
        Location::S3 {
            bucket,
            key,
            region,
        } => {
            let mut command = Command::new("aws");
            command.args(["s3api", "delete-object", "--bucket", &bucket, "--key", &key]);
            if let Some(region) = &region {
                command.args(["--region", region]);
            }
            let _ = command.output();
        }
        Location::File(path) => {
            if path.exists() {
                std::fs::remove_file(path)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(units: &[(&str, &[(&str, &str)])]) -> EnvironmentManifest {
        EnvironmentManifest {
            version: MANIFEST_VERSION,
            project: "demo".to_string(),
            environment: "pr-1".to_string(),
            workspace: "demo-pr-1".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            units: units
                .iter()
                .map(|(name, dependencies)| {
                    (
                        name.to_string(),
                        DeployedUnit {
                            path: name.to_string(),
                            state_key: Some(format!("{}/terraform.tfstate", name)),
                            dependencies: dependencies
                                .iter()
                                .map(|(unit, environment)| DeployedDependency {
                                    unit: unit.to_string(),
                                    environment: environment.to_string(),
                                })
                                .collect(),
                        },
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn merging_keeps_units_a_partial_deploy_did_not_touch() {
        let mut existing = manifest(&[("network", &[]), ("api", &[("network", "ephemeral.pr-1")])]);
        existing.merge(manifest(&[("api", &[("network", "stable.prod")])]));

        assert_eq!(existing.unit_names(), vec!["api", "network"]);
        assert_eq!(
            existing.units["api"].dependencies[0].environment, "stable.prod",
            "the newer record wins for units it covers"
        );
    }

    #[test]
    fn round_trips_through_a_file() {
        let directory = tempfile::TempDir::new().unwrap();
        let environment = crate::common::environment::ResolvedEnvironment::for_tests(
            "pr-1",
            "demo-pr-1",
            "local",
        );

        assert!(load(directory.path(), &environment).unwrap().is_none());

        save(
            directory.path(),
            &environment,
            manifest(&[("api", &[("network", "stable.prod")])]),
        )
        .unwrap();

        let loaded = load(directory.path(), &environment).unwrap().unwrap();
        assert_eq!(loaded.unit_names(), vec!["api"]);

        remove(directory.path(), &environment).unwrap();
        assert!(load(directory.path(), &environment).unwrap().is_none());
    }

    #[test]
    fn only_dependencies_from_elsewhere_become_overrides() {
        let environment = crate::common::environment::ResolvedEnvironment::for_tests(
            "pr-1",
            "demo-pr-1",
            "local",
        );

        let own = environment.reference();
        let recorded = manifest(&[("api", &[("network", "stable.prod"), ("db", own.as_str())])]);

        let overrides = recorded.dependency_overrides(&environment);
        assert_eq!(
            overrides.get("network").map(String::as_str),
            Some("stable.prod")
        );
        assert!(
            !overrides.contains_key("db"),
            "a dependency read from this environment needs no override"
        );
    }
}
