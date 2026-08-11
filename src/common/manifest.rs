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
use crate::common::environment::{BackendConfig, EnvironmentConfig, ResolvedEnvironment};
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

/// Where a backend's records are kept, as a set rather than one at a time, so
/// that the environments a project has can be listed and not only looked up.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Store {
    S3 {
        bucket: String,
        region: Option<String>,
    },
    Directory(PathBuf),
}

const S3_PREFIX: &str = "envie/manifests/";

/// What an AWS CLI failure was actually about, without its framing.
fn aws_failure(stderr: &[u8]) -> String {
    String::from_utf8_lossy(stderr)
        .lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .map(|line| {
            line.trim_start_matches("aws: ")
                .trim_start_matches("[ERROR]: ")
                .to_string()
        })
        .unwrap_or_else(|| "the aws CLI failed".to_string())
}

fn store(root: &Path, backend: &BackendConfig) -> Store {
    if backend.backend_type == "s3" {
        if let Some(bucket) = backend.config.get("bucket") {
            return Store::S3 {
                bucket: bucket.clone(),
                region: backend.config.get("region").cloned(),
            };
        }
    }

    Store::Directory(root.join(".envie").join("manifests"))
}

fn file_name(environment: &str) -> String {
    format!("{}.json", environment)
}

impl Store {
    fn key(&self, environment: &str) -> String {
        format!("{}{}", S3_PREFIX, file_name(environment))
    }

    /// How to refer to this store when telling someone where a record came from.
    fn describe(&self) -> String {
        match self {
            Store::S3 { bucket, .. } => format!("s3://{}/{}", bucket, S3_PREFIX),
            Store::Directory(directory) => directory.display().to_string(),
        }
    }

    fn region_arguments(&self) -> Vec<String> {
        match self {
            Store::S3 {
                region: Some(region),
                ..
            } => vec!["--region".to_string(), region.clone()],
            _ => Vec::new(),
        }
    }

    /// The environments this store holds a record for.
    ///
    /// The failure is a plain sentence rather than an `EnvieError` because it is
    /// shown to somebody who asked what environments exist, not raised at them.
    fn environment_names(&self) -> std::result::Result<Vec<String>, String> {
        match self {
            Store::Directory(directory) => {
                if !directory.exists() {
                    return Ok(Vec::new());
                }

                let mut names = Vec::new();
                for entry in std::fs::read_dir(directory).map_err(|e| e.to_string())? {
                    let path = entry.map_err(|e| e.to_string())?.path();
                    if path.extension().and_then(|e| e.to_str()) != Some("json") {
                        continue;
                    }
                    if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                        names.push(name.to_string());
                    }
                }
                names.sort();
                Ok(names)
            }
            Store::S3 { bucket, .. } => {
                let mut command = Command::new("aws");
                command.args([
                    "s3api",
                    "list-objects-v2",
                    "--bucket",
                    bucket,
                    "--prefix",
                    S3_PREFIX,
                    "--output",
                    "json",
                ]);
                command.args(self.region_arguments());

                let output = command
                    .output()
                    .map_err(|e| format!("the aws CLI could not be run ({})", e))?;
                if !output.status.success() {
                    return Err(aws_failure(&output.stderr));
                }

                let listing: serde_json::Value = serde_json::from_slice(&output.stdout)
                    .unwrap_or(serde_json::Value::Object(Default::default()));

                let mut names: Vec<String> = listing["Contents"]
                    .as_array()
                    .map(|contents| contents.as_slice())
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|object| object["Key"].as_str())
                    .filter_map(|key| key.strip_prefix(S3_PREFIX))
                    .filter_map(|name| name.strip_suffix(".json"))
                    .filter(|name| !name.is_empty())
                    .map(str::to_string)
                    .collect();
                names.sort();
                Ok(names)
            }
        }
    }

    fn read(&self, environment: &str) -> Result<Option<String>> {
        match self {
            Store::Directory(directory) => {
                let path = directory.join(file_name(environment));
                if !path.exists() {
                    return Ok(None);
                }
                Ok(Some(std::fs::read_to_string(path)?))
            }
            Store::S3 { bucket, .. } => {
                let destination = std::env::temp_dir().join(format!(
                    "envie-manifest-read-{}-{}.json",
                    environment,
                    std::process::id()
                ));

                let mut command = Command::new("aws");
                command.args([
                    "s3api",
                    "get-object",
                    "--bucket",
                    bucket,
                    "--key",
                    &self.key(environment),
                    &destination.to_string_lossy(),
                ]);
                command.args(self.region_arguments());

                let output = command.output()?;
                if !output.status.success() {
                    let _ = std::fs::remove_file(&destination);
                    return Ok(None);
                }

                let body = std::fs::read_to_string(&destination)?;
                let _ = std::fs::remove_file(&destination);
                Ok(Some(body))
            }
        }
    }
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

    write(root, environment, &merged)
}

/// Drop the units a teardown removed, and the environment itself once its record
/// describes nothing.
///
/// Without this, an environment that has been destroyed keeps looking deployed to
/// anyone asking what exists.
pub fn forget(root: &Path, environment: &ResolvedEnvironment, units: &[String]) -> Result<()> {
    let Some(mut existing) = load(root, environment)? else {
        return Ok(());
    };

    existing.units.retain(|name, _| !units.contains(name));

    if existing.units.is_empty() {
        return remove(root, environment);
    }

    existing.updated_at = chrono::Utc::now().to_rfc3339();
    write(root, environment, &existing)
}

fn write(
    root: &Path,
    environment: &ResolvedEnvironment,
    manifest: &EnvironmentManifest,
) -> Result<()> {
    let body = serde_json::to_string_pretty(manifest)
        .map_err(|e| EnvieError::ConfigError(format!("could not serialise the manifest: {}", e)))?;

    let store = store(root, &environment.backend);
    match &store {
        Store::S3 { bucket, .. } => {
            let key = store.key(&environment.name);
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
                bucket,
                "--key",
                &key,
                "--body",
                &temporary.to_string_lossy(),
                "--content-type",
                "application/json",
            ]);
            command.args(store.region_arguments());

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
        Store::Directory(directory) => {
            std::fs::create_dir_all(directory)?;
            std::fs::write(directory.join(file_name(&environment.name)), body)?;
        }
    }

    Ok(())
}

/// What Envie recorded for this environment, if anything.
///
/// A missing record is normal: the environment may predate this version of
/// Envie, or may never have been deployed.
pub fn load(root: &Path, environment: &ResolvedEnvironment) -> Result<Option<EnvironmentManifest>> {
    let body = match store(root, &environment.backend).read(&environment.name)? {
        Some(body) => body,
        None => return Ok(None),
    };

    // A record Envie cannot read is treated as absent rather than fatal: it must
    // never be the reason an environment cannot be destroyed.
    Ok(serde_json::from_str(&body).ok())
}

/// Forget an environment, once it no longer exists.
pub fn remove(root: &Path, environment: &ResolvedEnvironment) -> Result<()> {
    let store = store(root, &environment.backend);
    match &store {
        Store::S3 { bucket, .. } => {
            let mut command = Command::new("aws");
            command.args([
                "s3api",
                "delete-object",
                "--bucket",
                bucket,
                "--key",
                &store.key(&environment.name),
            ]);
            command.args(store.region_arguments());
            let _ = command.output();
        }
        Store::Directory(directory) => {
            let path = directory.join(file_name(&environment.name));
            if path.exists() {
                std::fs::remove_file(path)?;
            }
        }
    }
    Ok(())
}

/// An environment Envie has a record of.
#[derive(Debug, Clone)]
pub struct RecordedEnvironment {
    pub name: String,
    /// `None` when the record exists but cannot be read, which still tells us
    /// the environment is out there.
    pub manifest: Option<EnvironmentManifest>,
    /// Where the record was found, for output that has to explain itself.
    pub source: String,
}

/// Every environment recorded in the backends this project deploys to.
///
/// A backend that cannot be reached is reported rather than raised: not having
/// credentials for one bucket should still list the environments in the others,
/// and should never stop a project describing what it has configured.
pub fn recorded(
    root: &Path,
    environments: &EnvironmentConfig,
) -> (Vec<RecordedEnvironment>, Vec<String>) {
    let mut stores = vec![store(root, &environments.ephemeral.backend)];
    for stable in environments.stable.values() {
        stores.push(store(root, &stable.backend));
    }
    stores.sort();
    stores.dedup();

    let mut found: Vec<RecordedEnvironment> = Vec::new();
    let mut problems = Vec::new();

    for store in stores {
        let names = match store.environment_names() {
            Ok(names) => names,
            Err(error) => {
                problems.push(format!("could not read {}: {}", store.describe(), error));
                continue;
            }
        };

        for name in names {
            if found.iter().any(|existing| existing.name == name) {
                continue;
            }

            let manifest = store
                .read(&name)
                .ok()
                .flatten()
                .and_then(|body| serde_json::from_str(&body).ok());

            found.push(RecordedEnvironment {
                name,
                manifest,
                source: store.describe(),
            });
        }
    }

    found.sort_by(|a, b| a.name.cmp(&b.name));
    (found, problems)
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
    fn destroying_part_of_an_environment_leaves_the_rest_recorded() {
        let directory = tempfile::TempDir::new().unwrap();
        let environment = crate::common::environment::ResolvedEnvironment::for_tests(
            "pr-1",
            "demo-pr-1",
            "local",
        );
        save(
            directory.path(),
            &environment,
            manifest(&[("db", &[]), ("api", &[("db", "ephemeral.pr-1")])]),
        )
        .unwrap();

        forget(directory.path(), &environment, &["api".to_string()]).unwrap();

        let remaining = load(directory.path(), &environment).unwrap().unwrap();
        assert_eq!(remaining.unit_names(), vec!["db"]);
    }

    #[test]
    fn destroying_everything_forgets_the_environment() {
        let directory = tempfile::TempDir::new().unwrap();
        let environment = crate::common::environment::ResolvedEnvironment::for_tests(
            "pr-1",
            "demo-pr-1",
            "local",
        );
        save(
            directory.path(),
            &environment,
            manifest(&[("db", &[]), ("api", &[])]),
        )
        .unwrap();

        forget(
            directory.path(),
            &environment,
            &["db".to_string(), "api".to_string()],
        )
        .unwrap();

        assert!(
            load(directory.path(), &environment).unwrap().is_none(),
            "an environment with nothing left in it should not look deployed"
        );
    }

    #[test]
    fn forgetting_an_environment_that_was_never_recorded_is_not_an_error() {
        let directory = tempfile::TempDir::new().unwrap();
        let environment = crate::common::environment::ResolvedEnvironment::for_tests(
            "pr-1",
            "demo-pr-1",
            "local",
        );

        forget(directory.path(), &environment, &["api".to_string()]).unwrap();
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
