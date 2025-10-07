use crate::cli::args::*;
use crate::commands::*;
use crate::common::*;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct CommandHandler {
    working_directory: PathBuf,
}

impl CommandHandler {
    pub fn new() -> Self {
        Self {
            working_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    pub async fn handle_command(&self, command: Commands) -> Result<()> {
        match command {
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
                service,
                merge_request,
                environment,
                dry_run,
                no_prompt: _no_prompt,
                verbose,
            } => {
                let environments = self.parse_environments(environment)?;
                
                let options = DeployV2Options {
                    service_name: service,
                    merge_request,
                    environment_overrides: environments,
                    dry_run,
                    no_prompt: false,
                    verbose,
                };

                let deployer = DeployV2Command::new(self.working_directory.clone());
                deployer.execute(options).await
            }
            Commands::Destroy {
                merge_request,
                dry_run,
                verbose,
            } => {
                let options = DestroyOptions {
                    merge_request,
                    dry_run,
                    verbose,
                };

                let destroyer = DestroyCommand::new(self.working_directory.clone());
                destroyer.execute(options).await
            }
            Commands::Env { command } => {
                self.handle_env_command(command).await
            }
            Commands::Generate { env_file, file } => {
                let use_envie_output = file.is_none();
                let options = GenerateOptions {
                    env_file,
                    output_file: file,
                    use_envie_output,
                };

                let generator = GenerateCommand::new(self.working_directory.clone());
                generator.execute(options).await
            }
            Commands::List => {
                let lister = ListCommand::new(self.working_directory.clone());
                lister.list()
            }
            Commands::Output { file, verbose } => {
                let options = OutputOptions {
                    output_file: file.map(|p| p.to_string_lossy().to_string()),
                    verbose,
                };

                let output = OutputCommand::new(self.working_directory.clone());
                output.execute(options).await
            }
            Commands::Clean {
                service,
                upgrade,
                verbose,
            } => {
                let options = CleanOptions {
                    service_name: service,
                    upgrade,
                    verbose,
                };

                let cleaner = CleanCommand::new(self.working_directory.clone());
                cleaner.execute(options)
            }
            Commands::Show {
                service,
                modules,
                dependencies,
                verbose,
            } => {
                let options = ShowOptions {
                    service,
                    modules,
                    dependencies,
                    verbose,
                };

                let shower = ShowCommand::new(self.working_directory.clone());
                shower.execute(options)
            }
        }
    }

    async fn handle_env_command(&self, command: EnvCommands) -> Result<()> {
        match command {
            EnvCommands::Start {
                merge_request_id,
                quiet,
            } => {
                let options = EnvOptions {
                    merge_request_id,
                    quiet,
                };

                let env_cmd = EnvCommand::new(self.working_directory.clone());
                env_cmd.start(options).await
            }
            EnvCommands::Destroy {
                merge_request_id,
                quiet,
            } => {
                let options = EnvOptions {
                    merge_request_id: merge_request_id.unwrap_or_default(),
                    quiet,
                };

                let env_cmd = EnvCommand::new(self.working_directory.clone());
                env_cmd.destroy(options).await
            }
            EnvCommands::List => {
                let env_cmd = EnvCommand::new(self.working_directory.clone());
                env_cmd.list()
            }
            EnvCommands::Current => {
                let env_cmd = EnvCommand::new(self.working_directory.clone());
                env_cmd.current()
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
                return Err(EnvieError::ValidationError(
                    format!("Invalid environment format: {}. Expected format: key:value", env_arg)
                ));
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
        let temp_dir = TempDir::new().unwrap();
        let handler = CommandHandler::new();
        
        let env_args = vec![
            "service1:dev".to_string(),
            "service2:prod".to_string(),
        ];
        
        let result = handler.parse_environments(env_args).unwrap();
        assert_eq!(result.get("service1"), Some(&"dev".to_string()));
        assert_eq!(result.get("service2"), Some(&"prod".to_string()));
    }

    #[test]
    fn test_parse_environments_invalid_format() {
        let temp_dir = TempDir::new().unwrap();
        let handler = CommandHandler::new();
        
        let env_args = vec!["invalid_format".to_string()];
        
        let result = handler.parse_environments(env_args);
        assert!(result.is_err());
    }
}
