//! Fills a `.env` template with values from an environment's Terraform outputs.
//!
//! The template names outputs as `unit.output`, optionally reaching into a
//! structured output with further dots:
//!
//! ```text
//! API_URL=api.invoke_url
//! TABLE=db.table.name
//! ```

use crate::common::*;
use serde_json::Value;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct GenerateOptions {
    pub env_file: PathBuf,
    /// Read outputs from this file instead of the deployed environment.
    pub output_file: Option<PathBuf>,
    /// Which environment to read outputs from. Required unless `output_file` is set.
    pub env_id: Option<String>,
}

pub struct GenerateCommand {
    working_directory: PathBuf,
    output_manager: OutputManager,
}

impl GenerateCommand {
    pub fn new(working_directory: PathBuf) -> Self {
        Self {
            working_directory,
            output_manager: OutputManager::new(),
        }
    }

    pub async fn execute(&self, options: GenerateOptions) -> Result<()> {
        let terraform_output = match (&options.output_file, &options.env_id) {
            (Some(path), _) => self.read_outputs_from_file(path)?,
            (None, Some(env_id)) => self.read_outputs_from_environment(env_id)?,
            (None, None) => {
                return Err(EnvieError::ValidationError(
                    "--env is required, so it is clear which environment's outputs \
                     to use. Pass --file instead to read them from a file."
                        .to_string(),
                ))
            }
        };

        // Parse environment file
        let env_vars = self.parse_env_file(&options.env_file, &terraform_output)?;

        // Generate .env file
        self.generate_env_file(&env_vars)?;

        self.output_manager
            .print_green("Success: .env has been generated successfully!");

        Ok(())
    }

    fn read_outputs_from_environment(&self, env_id: &str) -> Result<Value> {
        self.output_manager
            .print_yellow(&format!("Reading outputs from {env_id}..."));

        let outputs = crate::commands::output::collect(&self.working_directory, env_id, None)?;
        if outputs.is_empty() {
            return Err(EnvieError::ValidationError(format!(
                "nothing is deployed in {env_id}, so there are no outputs to use"
            )));
        }

        Ok(serde_json::to_value(outputs)?)
    }

    fn read_outputs_from_file(&self, file_path: &PathBuf) -> Result<Value> {
        self.output_manager.print_yellow(&format!(
            "Reading Terraform outputs from: {}",
            file_path.display()
        ));

        if !file_path.exists() {
            return Err(EnvieError::FileSystemError(format!(
                "File '{}' does not exist",
                file_path.display()
            )));
        }

        let content = std::fs::read_to_string(file_path)?;
        if content.trim().is_empty() {
            return Err(EnvieError::FileSystemError(format!(
                "Failed to read data from '{}'",
                file_path.display()
            )));
        }

        Ok(serde_json::from_str(&content)?)
    }

    fn parse_env_file(&self, env_file: &PathBuf, terraform_output: &Value) -> Result<Vec<String>> {
        self.output_manager
            .print_yellow(&format!("Parsing {} ...", env_file.display()));

        if !env_file.exists() {
            return Err(EnvieError::FileSystemError(format!(
                "Environment file '{}' does not exist",
                env_file.display()
            )));
        }

        let content = std::fs::read_to_string(env_file)?;
        let mut env_vars = Vec::new();

        for line in content.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse key=value pairs
            if let Some((key, value)) = self.parse_env_line(line) {
                // A value only names an output if its first segment is one of the
                // units. Anything else is a literal the template wants kept, and
                // dropping those would quietly produce an incomplete .env.
                if !names_an_output(&value, terraform_output) {
                    env_vars.push(format!("{}=\"{}\"", key, value));
                    continue;
                }

                match self.extract_terraform_value(&value, terraform_output)? {
                    Some(terraform_value) => {
                        env_vars.push(format!("{}=\"{}\"", key, terraform_value))
                    }
                    None => self.output_manager.print_yellow(&format!(
                        "Warning: {} is not an output of {}, so {} was left out.",
                        value,
                        value.split('.').next().unwrap_or(&value),
                        key
                    )),
                }
            }
        }

        Ok(env_vars)
    }

    fn parse_env_line(&self, line: &str) -> Option<(String, String)> {
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim().to_string();
            let value = line[eq_pos + 1..].trim().to_string();

            // Remove quotes from value
            let value = value.trim_matches('"').to_string();

            Some((key, value))
        } else {
            None
        }
    }

    /// Look up `unit.output`, or `unit.output.attribute` for a structured output.
    fn extract_terraform_value(
        &self,
        reference: &str,
        terraform_output: &Value,
    ) -> Result<Option<String>> {
        let mut current = terraform_output;

        for segment in reference.split('.') {
            current = unwrap_terraform_value(current);
            match current.get(segment) {
                Some(next) => current = next,
                None => return Ok(None),
            }
        }

        Ok(Some(match unwrap_terraform_value(current) {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            other => other.to_string(),
        }))
    }

    fn generate_env_file(&self, env_vars: &[String]) -> Result<()> {
        // Check if running in CI
        if std::env::var("CI_PIPELINE_URL").is_ok() {
            self.output_manager
                .print_yellow("Running in CI, skipping .env clearing...");
        } else {
            self.output_manager.print_yellow("Clearing .env...");
            let env_file = self.working_directory.join(".env");
            if env_file.exists() {
                std::fs::write(&env_file, "")?;
            }
        }

        self.output_manager.print_yellow("Generating .env...");

        let env_file = self.working_directory.join(".env");
        let mut content = String::new();

        for var in env_vars {
            content.push_str(var);
            content.push('\n');
        }

        std::fs::write(&env_file, content)?;

        Ok(())
    }
}

/// Whether a template value refers to an output rather than being a literal.
fn names_an_output(value: &str, terraform_output: &Value) -> bool {
    value
        .split_once('.')
        .is_some_and(|(unit, _)| terraform_output.get(unit).is_some())
}

/// Steps through the `{ "value": ..., "type": ... }` wrapper that
/// `terraform output -json` puts around every output. `envie output` writes the
/// values directly, so both shapes can be read with the same reference.
///
/// Only a wrapper is stepped through: an output whose own value is an object
/// with a `value` attribute of its own keeps its other attributes, so it is left
/// alone.
fn unwrap_terraform_value(value: &Value) -> &Value {
    const WRAPPER_KEYS: [&str; 3] = ["value", "type", "sensitive"];

    match value.as_object() {
        Some(object)
            if object.contains_key("value")
                && object
                    .keys()
                    .all(|key| WRAPPER_KEYS.contains(&key.as_str())) =>
        {
            &object["value"]
        }
        _ => value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn command() -> (TempDir, GenerateCommand) {
        let tmp = TempDir::new().unwrap();
        let command = GenerateCommand::new(tmp.path().to_path_buf());
        (tmp, command)
    }

    /// The shape `envie output --format json` writes.
    fn combined() -> Value {
        serde_json::json!({
            "api": {
                "invoke_url": "https://example.execute-api.eu-west-1.amazonaws.com/pr-1",
                "endpoints": { "list": "GET /items" },
                "port": 8080
            },
            "db": { "table_name": "acme-pr-1-items" }
        })
    }

    #[test]
    fn an_output_is_read_from_the_combined_shape() {
        let (_tmp, command) = command();

        assert_eq!(
            command
                .extract_terraform_value("db.table_name", &combined())
                .unwrap(),
            Some("acme-pr-1-items".to_string())
        );
    }

    /// A structured output can be reached into with further dots.
    #[test]
    fn an_attribute_of_a_structured_output_is_reachable() {
        let (_tmp, command) = command();

        assert_eq!(
            command
                .extract_terraform_value("api.endpoints.list", &combined())
                .unwrap(),
            Some("GET /items".to_string())
        );
    }

    #[test]
    fn numbers_are_written_without_quotes_of_their_own() {
        let (_tmp, command) = command();

        assert_eq!(
            command
                .extract_terraform_value("api.port", &combined())
                .unwrap(),
            Some("8080".to_string())
        );
    }

    /// `--file` is usually handed the raw output of `terraform output -json`,
    /// which wraps every value.
    #[test]
    fn the_wrapper_terraform_puts_around_outputs_is_stepped_through() {
        let (_tmp, command) = command();
        let raw = serde_json::json!({
            "db": {
                "value": { "table_name": "acme-pr-1-items" },
                "type": ["object", { "table_name": "string" }]
            }
        });

        assert_eq!(
            command
                .extract_terraform_value("db.table_name", &raw)
                .unwrap(),
            Some("acme-pr-1-items".to_string())
        );
    }

    /// An output whose own value happens to have a `value` attribute must keep
    /// it, or reaching into it would silently return the wrong thing.
    #[test]
    fn an_output_with_its_own_value_attribute_is_not_unwrapped() {
        let (_tmp, command) = command();
        let outputs = serde_json::json!({
            "cfg": { "setting": { "value": "on", "source": "default" } }
        });

        assert_eq!(
            command
                .extract_terraform_value("cfg.setting.source", &outputs)
                .unwrap(),
            Some("default".to_string())
        );
    }

    #[test]
    fn an_output_that_does_not_exist_resolves_to_nothing() {
        let (_tmp, command) = command();

        assert_eq!(
            command
                .extract_terraform_value("db.missing", &combined())
                .unwrap(),
            None
        );
    }

    /// A template carries literals as well as references, and dropping those
    /// would produce a .env that is quietly missing variables.
    #[test]
    fn a_literal_is_told_apart_from_a_reference() {
        assert!(names_an_output("db.table_name", &combined()));
        assert!(!names_an_output("debug", &combined()));
        assert!(!names_an_output("eu-west-1", &combined()));
        assert!(!names_an_output("example.com", &combined()));
    }

    #[test]
    fn literals_are_kept_and_references_are_resolved() {
        let (tmp, command) = command();
        let template = tmp.path().join(".env.example");
        std::fs::write(
            &template,
            "# a comment\nTABLE=db.table_name\nREGION=eu-west-1\nMISSING=db.nope\n",
        )
        .unwrap();

        let vars = command.parse_env_file(&template, &combined()).unwrap();

        assert_eq!(
            vars,
            vec![
                "TABLE=\"acme-pr-1-items\"".to_string(),
                "REGION=\"eu-west-1\"".to_string(),
            ]
        );
    }

    #[test]
    fn generating_without_an_environment_or_a_file_is_refused() {
        let (tmp, command) = command();
        let template = tmp.path().join(".env.example");
        std::fs::write(&template, "TABLE=db.table_name\n").unwrap();

        let error = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(command.execute(GenerateOptions {
                env_file: template,
                output_file: None,
                env_id: None,
            }))
            .unwrap_err();

        assert!(error.to_string().contains("--env is required"), "{error}");
    }

    #[test]
    fn env_lines_are_parsed_with_and_without_quotes() {
        let (_tmp, command) = command();

        assert_eq!(
            command.parse_env_line("KEY=value"),
            Some(("KEY".to_string(), "value".to_string()))
        );
        assert_eq!(
            command.parse_env_line("KEY=\"quoted value\""),
            Some(("KEY".to_string(), "quoted value".to_string()))
        );
        assert_eq!(command.parse_env_line("KEY"), None);
        assert_eq!(command.parse_env_line(""), None);
    }
}
