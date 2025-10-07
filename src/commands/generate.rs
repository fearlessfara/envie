use crate::common::*;
use std::path::PathBuf;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct GenerateOptions {
    pub env_file: PathBuf,
    pub output_file: Option<PathBuf>,
    pub use_envie_output: bool,
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
        // Get terraform outputs
        let terraform_output = if options.use_envie_output {
            self.get_envie_output().await?
        } else {
            self.get_terraform_output_from_file(options.output_file.as_ref().unwrap()).await?
        };

        // Parse environment file
        let env_vars = self.parse_env_file(&options.env_file, &terraform_output)?;

        // Generate .env file
        self.generate_env_file(&env_vars).await?;

        self.output_manager.print_green("Success: .env has been generated successfully!");

        Ok(())
    }

    async fn get_envie_output(&self) -> Result<Value> {
        self.output_manager.print_yellow("Calling `envie output`...");
        
        // This would call the envie output command
        // For now, we'll return a placeholder
        Ok(serde_json::json!({
            "example_key": {
                "value": "example_value"
            }
        }))
    }

    async fn get_terraform_output_from_file(&self, file_path: &PathBuf) -> Result<Value> {
        self.output_manager.print_yellow(&format!("Reading Terraform outputs from: {}", file_path.display()));
        
        if !file_path.exists() {
            return Err(EnvieError::FileSystemError(
                format!("File '{}' does not exist", file_path.display())
            ));
        }

        let content = std::fs::read_to_string(file_path)?;
        if content.trim().is_empty() {
            return Err(EnvieError::FileSystemError(
                format!("Failed to read data from '{}'", file_path.display())
            ));
        }

        let parsed: Value = serde_json::from_str(&content)?;
        Ok(parsed)
    }

    fn parse_env_file(&self, env_file: &PathBuf, terraform_output: &Value) -> Result<Vec<String>> {
        self.output_manager.print_yellow(&format!("Parsing {} ...", env_file.display()));
        
        if !env_file.exists() {
            return Err(EnvieError::FileSystemError(
                format!("Environment file '{}' does not exist", env_file.display())
            ));
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
                if let Some(terraform_value) = self.extract_terraform_value(&value, terraform_output)? {
                    env_vars.push(format!("{}=\"{}\"", key, terraform_value));
                } else {
                    self.output_manager.print_yellow(&format!("Warning: Failed to parse {}={} from Terraform outputs.", key, value));
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

    fn extract_terraform_value(&self, value: &str, terraform_output: &Value) -> Result<Option<String>> {
        // Handle hierarchical references (e.g., "service.component.attribute")
        let parts: Vec<&str> = value.split('.').collect();
        
        if parts.len() < 2 {
            return Ok(None);
        }

        let first_key = parts[0];
        let remaining_path = parts[1..].join(".");

        let terraform_var = if remaining_path.contains('.') {
            format!("{}.{}", first_key, remaining_path)
        } else {
            format!("{}.value", first_key)
        };

        // Extract value from JSON using the terraform variable path
        let mut current_value = terraform_output;
        for part in terraform_var.split('.') {
            if let Some(obj) = current_value.as_object() {
                if let Some(next_value) = obj.get(part) {
                    current_value = next_value;
                } else {
                    return Ok(None);
                }
            } else {
                return Ok(None);
            }
        }

        // Convert to string
        match current_value {
            Value::String(s) => Ok(Some(s.clone())),
            Value::Number(n) => Ok(Some(n.to_string())),
            Value::Bool(b) => Ok(Some(b.to_string())),
            _ => Ok(Some(current_value.to_string())),
        }
    }

    async fn generate_env_file(&self, env_vars: &[String]) -> Result<()> {
        // Check if running in CI
        if std::env::var("CI_PIPELINE_URL").is_ok() {
            self.output_manager.print_yellow("Running in CI, skipping .env clearing...");
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_generate_command_creation() {
        let temp_dir = TempDir::new().unwrap();
        let generator = GenerateCommand::new(temp_dir.path().to_path_buf());
        assert_eq!(generator.working_directory, temp_dir.path());
    }

    #[test]
    fn test_parse_env_line() {
        let temp_dir = TempDir::new().unwrap();
        let generator = GenerateCommand::new(temp_dir.path().to_path_buf());
        
        // Valid lines
        assert_eq!(
            generator.parse_env_line("KEY=value"),
            Some(("KEY".to_string(), "value".to_string()))
        );
        assert_eq!(
            generator.parse_env_line("KEY=\"quoted value\""),
            Some(("KEY".to_string(), "quoted value".to_string()))
        );
        
        // Invalid lines
        assert_eq!(generator.parse_env_line("KEY"), None);
        assert_eq!(generator.parse_env_line(""), None);
    }

    #[test]
    fn test_extract_terraform_value() {
        let temp_dir = TempDir::new().unwrap();
        let generator = GenerateCommand::new(temp_dir.path().to_path_buf());
        
        let terraform_output = serde_json::json!({
            "service": {
                "component": {
                    "value": "test_value"
                }
            }
        });
        
        let result = generator.extract_terraform_value("service.component", &terraform_output).unwrap();
        assert_eq!(result, Some("test_value".to_string()));
    }

    #[test]
    fn test_extract_terraform_value_missing() {
        let temp_dir = TempDir::new().unwrap();
        let generator = GenerateCommand::new(temp_dir.path().to_path_buf());
        
        let terraform_output = serde_json::json!({
            "service": {
                "component": {
                    "value": "test_value"
                }
            }
        });
        
        let result = generator.extract_terraform_value("missing.key", &terraform_output).unwrap();
        assert_eq!(result, None);
    }
}
