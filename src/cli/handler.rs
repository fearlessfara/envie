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
            Commands::New {
                name,
                template,
                path,
                modules,
                no_prompt,
                verbose,
            } => {
                let options = NewOptions {
                    name,
                    template,
                    path,
                    modules,
                    no_prompt,
                    verbose,
                };

                let new_command = NewCommand::new(self.working_directory.clone());
                new_command.execute(options).await
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
            Commands::Plan {
                unit,
                env,
                environment,
                verbose,
            } => {
                // Plan is just an alias for deploy --dry-run
                let environments = self.parse_environments(environment)?;

                let options = DeployOptions {
                    unit_name: unit,
                    env_id: env,
                    environment_overrides: environments,
                    dry_run: true,  // Always use dry-run for plan
                    no_prompt: true, // No prompts needed for preview
                    verbose,
                };

                let deployer = DeployCommand::new(self.working_directory.clone());
                deployer.execute(options).await
            }
            Commands::Destroy {
                unit,
                env,
                dry_run,
                verbose,
            } => {
                let options = DestroyOptions {
                    unit_name: unit,
                    env_id: env,
                    dry_run,
                    verbose,
                };

                let destroyer = DestroyCommand::new(self.working_directory.clone());
                destroyer.execute(options).await
            }
            Commands::Delete {
                unit: _,
                env,
                dry_run,
                no_prompt,
                verbose,
            } => {
                let options = DeleteOptions {
                    env_id: env,
                    dry_run,
                    no_prompt,
                    verbose,
                };

                let deleter = DeleteCommand::new(self.working_directory.clone());
                deleter.execute(options).await
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
            Commands::Output { env, unit, file, format, verbose: _ } => {
                let output_format = match format.as_str() {
                    "json" => crate::commands::output::OutputFormat::Json,
                    "table" | _ => crate::commands::output::OutputFormat::Table,
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
                upgrade,
                verbose: _,
            } => {
                let options = CleanOptions {
                    unit_name: unit,
                    upgrade,
                };

                let cleaner = CleanCommand::new(self.working_directory.clone());
                cleaner.execute(options)
            }
            Commands::Show {
                unit,
                modules: _,
                dependencies: _,
                verbose,
            } => {
                let options = ShowOptions {
                    unit,
                    verbose,
                };

                let shower = ShowCommand::new(self.working_directory.clone());
                shower.execute(options)
            }
            Commands::Doctor { verbose } => {
                let options = DoctorOptions { verbose };

                let doctor = DoctorCommand::new(self.working_directory.clone());
                doctor.execute(options)
            }
        }
    }

    async fn handle_env_command(&self, command: EnvCommands) -> Result<()> {
        match command {
            EnvCommands::Start {
                env_id,
                quiet: _,
            } => {
                let options = EnvOptions {
                    merge_request_id: env_id,
                };

                let env_cmd = EnvCommand::new(self.working_directory.clone());
                env_cmd.start(options).await
            }
            EnvCommands::Destroy {
                env_id,
                quiet: _,
            } => {
                let options = EnvOptions {
                    merge_request_id: env_id.unwrap_or_default(),
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
            EnvCommands::TestDiscovery => {
                let test_cmd = TestDiscoveryCommand::new(self.working_directory.clone());
                test_cmd.execute()
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
                let mut error_msg = format!("❌ Invalid environment override format: '{}'\n\n", env_arg);
                error_msg.push_str("💡 Expected format:\n");
                error_msg.push_str("   -E <unit>:<environment>\n\n");
                error_msg.push_str("💡 Examples:\n");
                error_msg.push_str("   -E database:stable.sandbox\n");
                error_msg.push_str("   -E networking:stable.production\n");
                error_msg.push_str("   -E api:ephemeral\n\n");
                error_msg.push_str("💡 Common mistakes:\n");
                error_msg.push_str("   ✗ -E stable.sandbox (missing unit name)\n");
                error_msg.push_str("   ✓ -E database:stable.sandbox\n");

                return Err(EnvieError::ValidationError(error_msg));
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
