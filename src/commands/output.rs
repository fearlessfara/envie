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
    Yaml,
    Env,
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
                let body = format!("{}\n", serde_json::to_string_pretty(&by_unit)?);
                self.write_payload(options.output_file.as_deref(), &body)?;
            }
            OutputFormat::Yaml => {
                let body = serde_yaml::to_string(&by_unit)?;
                self.write_payload(options.output_file.as_deref(), &body)?;
            }
            OutputFormat::Env => {
                let body = render_env_file(&by_unit);
                self.write_payload(options.output_file.as_deref(), &body)?;
            }
            OutputFormat::Table => {
                self.print_table(&by_unit, &options.env_id);
                if let Some(path) = &options.output_file {
                    let body = format!("{}\n", serde_json::to_string_pretty(&by_unit)?);
                    self.write_payload(Some(path), &body)?;
                }
            }
        }

        Ok(())
    }

    /// Writes machine-readable output to a file or stdout. Status messages go
    /// through OutputManager so redirected stdout stays a clean payload.
    fn write_payload(&self, path: Option<&str>, body: &str) -> Result<()> {
        if let Some(path) = path {
            std::fs::write(path, body)?;
            self.output_manager
                .print_green(&format!("✅ Outputs written to {path}"));
        } else {
            print!("{body}");
            if !body.ends_with('\n') {
                println!();
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

/// Flat `.env` body: `UNIT_OUTPUT="value"` per Terraform output.
fn render_env_file(by_unit: &CombinedOutputs) -> String {
    let mut lines = Vec::new();
    for (unit, outputs) in by_unit {
        for (output, value) in outputs {
            lines.push(format!(
                "{}=\"{}\"",
                env_key(unit, output),
                env_value(value)
            ));
        }
    }
    let mut body = lines.join("\n");
    if !body.is_empty() {
        body.push('\n');
    }
    body
}

/// `{UNIT}_{OUTPUT}` uppercased, with non-alphanumeric characters as `_`.
fn env_key(unit: &str, output: &str) -> String {
    let raw = format!("{unit}_{output}");
    raw.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn env_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
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

    #[test]
    fn env_keys_uppercased_with_non_alnum_as_underscores() {
        assert_eq!(env_key("api", "invoke_url"), "API_INVOKE_URL");
        assert_eq!(env_key("services/api", "url"), "SERVICES_API_URL");
        assert_eq!(env_key("db", "table-name"), "DB_TABLE_NAME");
    }

    #[test]
    fn env_file_renders_flat_quoted_lines() {
        let mut by_unit = CombinedOutputs::new();
        by_unit.insert(
            "api".to_string(),
            BTreeMap::from([
                ("function_name".to_string(), serde_json::json!("pr-1-api")),
                (
                    "invoke_url".to_string(),
                    serde_json::json!("https://example.com"),
                ),
            ]),
        );
        by_unit.insert(
            "db".to_string(),
            BTreeMap::from([("table_name".to_string(), serde_json::json!("pr-1-items"))]),
        );

        assert_eq!(
            render_env_file(&by_unit),
            "API_FUNCTION_NAME=\"pr-1-api\"\n\
             API_INVOKE_URL=\"https://example.com\"\n\
             DB_TABLE_NAME=\"pr-1-items\"\n"
        );
    }

    #[test]
    fn env_file_serializes_structured_values_as_json() {
        let mut by_unit = CombinedOutputs::new();
        by_unit.insert(
            "api".to_string(),
            BTreeMap::from([(
                "tags".to_string(),
                serde_json::json!({"env": "pr-1", "team": "platform"}),
            )]),
        );

        let body = render_env_file(&by_unit);
        assert_eq!(
            body,
            "API_TAGS=\"{\"env\":\"pr-1\",\"team\":\"platform\"}\"\n"
        );
    }

    #[test]
    fn yaml_preserves_the_nested_unit_output_shape() {
        let mut by_unit = CombinedOutputs::new();
        by_unit.insert(
            "api".to_string(),
            BTreeMap::from([("url".to_string(), serde_json::json!("https://x"))]),
        );

        let yaml = serde_yaml::to_string(&by_unit).unwrap();
        let parsed: CombinedOutputs = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed["api"]["url"], serde_json::json!("https://x"));
    }
}
