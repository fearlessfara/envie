use crate::cli::args::*;
use crate::commands::*;
use crate::common::*;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct CommandHandler {
    working_directory: PathBuf,
}

impl Default for CommandHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandHandler {
    pub fn new() -> Self {
        Self {
            working_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    pub async fn handle_command(&self, command: Commands) -> Result<()> {
        match command {
            Commands::Adopt {
                name,
                environment,
                dry_run,
                force,
                verbose,
            } => {
                let options = AdoptOptions {
                    project_name: name,
                    environments: environment,
                    dry_run,
                    force,
                    verbose,
                };

                let adopt_command = AdoptCommand::new(self.working_directory.clone());
                adopt_command.execute(options)
            }
            Commands::Init {
                name,
                description,
                no_prompt,
                verbose,
            } => {
                let options = InitOptions {
                    name,
                    description,
                    no_prompt,
                    verbose,
                };

                let init_command = InitCommand::new(self.working_directory.clone());
                init_command.execute(options).await
            }
            Commands::Deploy {
                unit,
                env,
                environment,
                dry_run,
                no_prompt,
                verbose,
            } => {
                let environments = self.parse_environments(environment)?;

                let options = DeployOptions {
                    unit_name: unit,
                    env_id: env,
                    environment_overrides: environments,
                    dry_run,
                    no_prompt,
                    verbose,
                };

                let deployer = DeployCommand::new(self.working_directory.clone());
                deployer.execute(options).await
            }
            Commands::Destroy {
                unit,
                env,
                environment,
                dry_run,
                no_prompt,
                verbose,
            } => {
                let options = DestroyOptions {
                    unit_name: unit,
                    env_id: env,
                    environment_overrides: self.parse_environments(environment)?,
                    dry_run,
                    no_prompt,
                    verbose,
                };

                let destroyer = DestroyCommand::new(self.working_directory.clone());
                destroyer.execute(options).await
            }
            Commands::Delete {
                unit: _,
                env,
                environment,
                dry_run,
                no_prompt,
                verbose,
            } => {
                let options = DeleteOptions {
                    env_id: env,
                    environment_overrides: self.parse_environments(environment)?,
                    dry_run,
                    no_prompt,
                    verbose,
                };

                let deleter = DeleteCommand::new(self.working_directory.clone());
                deleter.execute(options).await
            }
            Commands::Generate {
                env,
                env_file,
                file,
            } => {
                let options = GenerateOptions {
                    env_file,
                    output_file: file,
                    env_id: env,
                };

                let generator = GenerateCommand::new(self.working_directory.clone());
                generator.execute(options).await
            }
            Commands::List { json } => {
                let lister = ListCommand::new(self.working_directory.clone());
                lister.execute(ListOptions { json })
            }
            Commands::Output {
                env,
                unit,
                file,
                format,
                verbose: _,
            } => {
                let output_format = match format.as_str() {
                    "json" => crate::commands::output::OutputFormat::Json,
                    _ => crate::commands::output::OutputFormat::Table,
                };

                let options = OutputOptions {
                    env_id: env,
                    unit_name: unit,
                    output_file: file.map(|p| p.to_string_lossy().to_string()),
                    format: output_format,
                };

                let output = OutputCommand::new(self.working_directory.clone());
                output.execute(options).await
            }
            Commands::Clean {
                unit,
                deep,
                verbose: _,
            } => {
                let options = CleanOptions {
                    unit_name: unit,
                    deep,
                };

                let cleaner = CleanCommand::new(self.working_directory.clone());
                cleaner.execute(options)
            }
            Commands::Show { unit, verbose } => {
                let options = ShowOptions { unit, verbose };

                let shower = ShowCommand::new(self.working_directory.clone());
                shower.execute(options)
            }
        }
    }

    fn parse_environments(&self, environment_args: Vec<String>) -> Result<HashMap<String, String>> {
        let mut environments = HashMap::new();

        for env_arg in environment_args {
            if let Some((key, value)) = env_arg.split_once(':') {
                if key == "default" {
                    // Handle default environment
                    // This would be stored separately in a real implementation
                } else {
                    environments.insert(key.to_string(), value.to_string());
                }
            } else {
                return Err(EnvieError::ValidationError(format!(
                    "Invalid environment format: {}. Expected format: key:value",
                    env_arg
                )));
            }
        }

        Ok(environments)
    }

    // TUI functionality will be implemented later
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_command_handler_creation() {
        let handler = CommandHandler::new();
        assert!(handler.working_directory.exists());
    }

    #[test]
    fn test_parse_environments() {
        let _temp_dir = TempDir::new().unwrap();
        let handler = CommandHandler::new();

        let env_args = vec!["service1:dev".to_string(), "service2:prod".to_string()];

        let result = handler.parse_environments(env_args).unwrap();
        assert_eq!(result.get("service1"), Some(&"dev".to_string()));
        assert_eq!(result.get("service2"), Some(&"prod".to_string()));
    }

    #[test]
    fn test_parse_environments_invalid_format() {
        let _temp_dir = TempDir::new().unwrap();
        let handler = CommandHandler::new();

        let env_args = vec!["invalid_format".to_string()];

        let result = handler.parse_environments(env_args);
        assert!(result.is_err());
    }
}
