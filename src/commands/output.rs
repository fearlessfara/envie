//! Collects the Terraform outputs of every unit in one environment.
//!
//! The environment is resolved through the planner, the same way `deploy` and
//! `destroy` resolve it. Deriving the workspace here instead would go wrong for
//! exactly the repositories Envie is meant to adopt, whose long-lived
//! environments live in Terraform's `default` workspace.

use crate::common::deployment::{PlanRequest, Planner, WorkspaceMode};
use crate::common::project::Project;
use crate::common::*;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct OutputOptions {
    pub output_file: Option<String>,
    pub env_id: String,
    pub unit_name: Option<String>,
    pub format: OutputFormat,
}

#[derive(Debug, Clone)]
pub enum OutputFormat {
    Json,
    Table,
}

pub struct OutputCommand {
    working_directory: PathBuf,
    output_manager: OutputManager,
}

impl OutputCommand {
    pub fn new(working_directory: PathBuf) -> Self {
        Self {
            working_directory,
            output_manager: OutputManager::new(),
        }
    }

    pub async fn execute(&self, options: OutputOptions) -> Result<()> {
        let by_unit = collect(
            &self.working_directory,
            &options.env_id,
            options.unit_name.as_deref(),
        )?;

        if by_unit.is_empty() {
            self.output_manager
                .print_yellow(&format!("Nothing is deployed in {}.", options.env_id));
            return Ok(());
        }

        match options.format {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(&by_unit)?;
                if let Some(path) = &options.output_file {
                    std::fs::write(path, format!("{json}\n"))?;
                    self.output_manager
                        .print_green(&format!("✅ Outputs written to {path}"));
                } else {
                    println!("{json}");
                }
            }
            OutputFormat::Table => {
                self.print_table(&by_unit, &options.env_id);
                if let Some(path) = &options.output_file {
                    std::fs::write(
                        path,
                        format!("{}\n", serde_json::to_string_pretty(&by_unit)?),
                    )?;
                    self.output_manager
                        .print_green(&format!("✅ Outputs written to {path}"));
                }
            }
        }

        Ok(())
    }

    fn print_table(
        &self,
        by_unit: &BTreeMap<String, BTreeMap<String, serde_json::Value>>,
        environment: &str,
    ) {
        self.output_manager
            .print_green(&format!("\n📊 Outputs for {environment}\n"));

        if by_unit.is_empty() {
            println!("  (no outputs)");
            return;
        }

        for (unit, outputs) in by_unit {
            println!("┌─ {unit} ─────────────────────────────────");
            for (key, value) in outputs {
                println!("│  {}: {}", key, render_value(value));
            }
            println!("└────────────────────────────────────────\n");
        }
    }
}

/// Every unit's Terraform outputs in one environment, keyed by unit name.
///
/// Shared with `envie generate`, so that the values written into a `.env` are
/// the ones `envie output` reports and both agree on where state lives.
pub type CombinedOutputs = BTreeMap<String, BTreeMap<String, serde_json::Value>>;

/// Reads the Terraform outputs of everything deployed in one environment.
///
/// Units the environment never deployed are skipped rather than reported as
/// failures: a repository usually has more units than any one environment used.
pub fn collect(
    working_directory: &std::path::Path,
    environment: &str,
    unit: Option<&str>,
) -> Result<CombinedOutputs> {
    let planner = Planner::new(Project::discover(working_directory)?)?;

    // Teardown planning replays what was actually deployed.
    let plan = planner.plan_teardown(&PlanRequest {
        environment: environment.to_string(),
        unit: unit.map(str::to_string),
        environment_overrides: HashMap::new(),
        include_dependencies: false,
        no_prompt: true,
        verbose: false,
    })?;

    let mut by_unit = CombinedOutputs::new();

    for planned in &plan.units {
        let prepared = planned.prepare(
            &plan.project_name,
            &plan.environment,
            WorkspaceMode::RequireExisting,
            false,
        )?;

        let Some(terraform) = prepared else {
            continue;
        };

        let outputs = terraform.output_json()?;
        if outputs.is_empty() {
            continue;
        }

        by_unit.insert(
            planned.name.clone(),
            outputs
                .into_iter()
                .map(|(key, output)| (key, output.value))
                .collect(),
        );
    }

    Ok(by_unit)
}

/// Renders an output for a terminal, leaving strings unquoted and anything
/// structured as the JSON it is.
fn render_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            serde_json::to_string(value).unwrap_or_else(|_| "...".to_string())
        }
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn a_directory_that_is_not_a_project_is_reported_as_such() {
        let tmp = TempDir::new().unwrap();

        let error = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(
                OutputCommand::new(tmp.path().to_path_buf()).execute(OutputOptions {
                    output_file: None,
                    env_id: "pr-1".to_string(),
                    unit_name: None,
                    format: OutputFormat::Table,
                }),
            )
            .unwrap_err();

        assert!(!error.to_string().is_empty());
    }

    #[test]
    fn strings_print_without_quotes_and_structures_print_as_json() {
        assert_eq!(
            render_value(&serde_json::json!("arn:aws:iam::x")),
            "arn:aws:iam::x"
        );
        assert_eq!(render_value(&serde_json::json!(3)), "3");
        assert_eq!(
            render_value(&serde_json::json!(["a", "b"])),
            "[\"a\",\"b\"]"
        );
    }
}
