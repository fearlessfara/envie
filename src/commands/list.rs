//! Which environments this project has.
//!
//! Two questions need answering, and neither can be answered from the repository
//! alone. The declared long-lived environments are in `workspace.envie.yaml`, but
//! the ephemeral ones exist only because somebody deployed them — often somebody
//! else, from another machine. So the deployment records Envie keeps in the
//! backend are read too, and the two are shown together.
//!
//! Nothing here touches Terraform or the environments themselves: listing what
//! exists must never be a reason for anything to change.

use crate::common::environment::BackendConfig;
use crate::common::project::Project;
use crate::common::{manifest, OutputManager, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct ListOptions {
    pub json: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EnvironmentKind {
    /// Declared in `workspace.envie.yaml` and meant to stay.
    Stable,
    /// Brought into being by a deploy, and expected to be deleted again.
    Ephemeral,
}

/// What the last deploy of an environment recorded.
#[derive(Debug, Clone, Serialize)]
pub struct Deployment {
    pub units: Vec<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnvironmentSummary {
    pub name: String,
    pub kind: EnvironmentKind,
    pub workspace: String,
    pub backend: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deployment: Option<Deployment>,
}

impl EnvironmentSummary {
    pub fn is_stable(&self) -> bool {
        self.kind == EnvironmentKind::Stable
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub project: String,
    pub environments: Vec<EnvironmentSummary>,
    /// Anything that made the list incomplete, so it never passes itself off as
    /// the whole picture.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub problems: Vec<String>,
}

impl Summary {
    pub fn get(&self, name: &str) -> Option<&EnvironmentSummary> {
        self.environments
            .iter()
            .find(|environment| environment.name == name)
    }
}

/// Gather the environments of the project `working_directory` belongs to.
pub fn summarise(working_directory: &Path) -> Result<Summary> {
    let project = Project::discover(working_directory)?;
    let configured = project.environments();
    let resolver = project.resolver("");
    let (records, mut problems) = manifest::recorded(&project.root, &configured);

    for record in &records {
        if record.manifest.is_none() {
            problems.push(format!(
                "the record for '{}' in {} could not be read, so its units are unknown",
                record.name, record.source
            ));
        }
    }

    let record_for = |name: &str| records.iter().find(|record| record.name == name);
    let deployment_of = |name: &str| {
        record_for(name)
            .and_then(|record| record.manifest.as_ref())
            .map(|manifest| Deployment {
                units: manifest.unit_names(),
                updated_at: manifest.updated_at.clone(),
            })
    };

    let mut environments = Vec::new();

    let mut stable_names: Vec<&String> = configured.stable.keys().collect();
    stable_names.sort();
    for name in stable_names {
        let stable = &configured.stable[name];
        environments.push(EnvironmentSummary {
            name: name.clone(),
            kind: EnvironmentKind::Stable,
            workspace: stable.workspace.clone(),
            backend: describe(&stable.backend),
            description: Some(stable.description.clone()).filter(|d| !d.is_empty()),
            deployment: deployment_of(name),
        });
    }

    for record in &records {
        if configured.stable.contains_key(&record.name) {
            continue;
        }

        let workspace = record
            .manifest
            .as_ref()
            .map(|manifest| manifest.workspace.clone())
            .unwrap_or_else(|| resolver.ephemeral_workspace(&record.name));

        environments.push(EnvironmentSummary {
            name: record.name.clone(),
            kind: EnvironmentKind::Ephemeral,
            workspace,
            backend: describe(&configured.ephemeral.backend),
            description: None,
            deployment: deployment_of(&record.name),
        });
    }

    Ok(Summary {
        project: project.name(),
        environments,
        problems,
    })
}

fn describe(backend: &BackendConfig) -> String {
    match (backend.backend_type.as_str(), backend.config.get("bucket")) {
        ("s3", Some(bucket)) => format!("s3://{}", bucket),
        _ => backend.backend_type.clone(),
    }
}

/// A timestamp as somebody reading a terminal would rather see it.
fn when(updated_at: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(updated_at)
        .map(|moment| {
            moment
                .with_timezone(&chrono::Utc)
                .format("%Y-%m-%d %H:%M UTC")
                .to_string()
        })
        .unwrap_or_else(|_| updated_at.to_string())
}

pub struct ListCommand {
    working_directory: PathBuf,
    output_manager: OutputManager,
}

impl ListCommand {
    pub fn new(working_directory: PathBuf) -> Self {
        Self {
            working_directory,
            output_manager: OutputManager::new(),
        }
    }

    pub fn execute(&self, options: ListOptions) -> Result<()> {
        let summary = summarise(&self.working_directory)?;

        if options.json {
            println!("{}", serde_json::to_string_pretty(&summary)?);
            return Ok(());
        }

        self.print(&summary);
        Ok(())
    }

    fn print(&self, summary: &Summary) {
        self.output_manager
            .print_green(&format!("📋 Environments in {}\n", summary.project));

        let (stable, ephemeral): (Vec<_>, Vec<_>) = summary
            .environments
            .iter()
            .partition(|environment| environment.is_stable());

        let width = summary
            .environments
            .iter()
            .map(|environment| environment.name.len())
            .max()
            .unwrap_or(0)
            .max(4);

        self.output_manager.print_blue("Long-lived");
        if stable.is_empty() {
            println!("  none declared in workspace.envie.yaml");
        }
        for environment in &stable {
            self.print_environment(environment, width);
        }

        println!();
        self.output_manager.print_blue("Ephemeral");
        if ephemeral.is_empty() {
            println!("  none deployed");
        }
        for environment in &ephemeral {
            self.print_environment(environment, width);
        }

        if !summary.problems.is_empty() {
            println!();
            for problem in &summary.problems {
                self.output_manager
                    .print_yellow(&format!("⚠️  {}", problem));
            }
        }

        println!("\nUnits and their dependencies: envie show");
    }

    fn print_environment(&self, environment: &EnvironmentSummary, width: usize) {
        let state = match &environment.deployment {
            Some(deployment) => format!(
                "deployed {} ({})",
                when(&deployment.updated_at),
                deployment.units.join(", ")
            ),
            None => "no deployment recorded".to_string(),
        };

        println!("  {:width$}  {}", environment.name, state, width = width);
        println!(
            "  {:width$}  workspace {}, state in {}",
            "",
            environment.workspace,
            environment.backend,
            width = width
        );
        if let Some(description) = &environment.description {
            println!("  {:width$}  {}", "", description, width = width);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_are_shortened_but_never_lost() {
        assert_eq!(
            when("2026-08-11T22:03:04.123456+00:00"),
            "2026-08-11 22:03 UTC"
        );
        assert_eq!(when("not a timestamp"), "not a timestamp");
    }
}
