use crate::cli::args::*;
use crate::common::*;
use crate::arya::*;
use crate::braavos::*;
use crate::stark::*;
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
            Commands::Arya { command } => self.handle_arya_command(command).await,
            Commands::Braavos { command } => self.handle_braavos_command(command).await,
            Commands::Stark { command } => self.handle_stark_command(command).await,
        }
    }

    async fn handle_arya_command(&self, command: AryaCommands) -> Result<()> {
        match command {
            AryaCommands::Start {
                service,
                merge_request,
                config,
                config_file,
                environment,
                dry_run,
                no_prompt,
                verbose,
            } => {
                let config_data = self.get_config_data(config, config_file)?;
                let environments = self.parse_environments(environment)?;
                
                let options = DeployOptions {
                    service_name: service,
                    merge_request,
                    config_data,
                    environments,
                    default_env: None,
                    dry_run,
                    no_prompt,
                    verbose,
                };

                let deployer = AryaDeployer::new(self.working_directory.clone());
                deployer.deploy(options).await
            }
            AryaCommands::Destroy {
                merge_request,
                dry_run,
                verbose,
            } => {
                let options = DestroyOptions {
                    merge_request,
                    dry_run,
                    verbose,
                };

                let destroyer = AryaDestroyer::new(self.working_directory.clone());
                destroyer.destroy(options).await
            }
            AryaCommands::List => {
                self.list_arya_environments().await
            }
            AryaCommands::Current => {
                self.current_arya_environment().await
            }
            AryaCommands::Output { file, verbose } => {
                let options = OutputOptions {
                    output_file: file.map(|p| p.to_string_lossy().to_string()),
                    verbose,
                };

                let output = AryaOutput::new(self.working_directory.clone());
                output.get_output(options).await
            }
            AryaCommands::Clean {
                service,
                upgrade,
                verbose,
            } => {
                let options = CleanOptions {
                    service_name: service,
                    upgrade,
                    verbose,
                };

                let cleaner = AryaCleaner::new(self.working_directory.clone());
                cleaner.clean(options)
            }
        }
    }

    async fn handle_braavos_command(&self, command: BraavosCommands) -> Result<()> {
        match command {
            BraavosCommands::Start {
                merge_request_id,
                quiet,
            } => {
                let options = StartOptions {
                    merge_request_id,
                    quiet,
                };

                let starter = BraavosStarter::new(self.working_directory.clone());
                starter.start(options).await
            }
            BraavosCommands::Destroy {
                merge_request_id,
                quiet,
            } => {
                let options = DestroyOptions {
                    merge_request_id,
                    quiet,
                };

                let destroyer = BraavosDestroyer::new(self.working_directory.clone());
                destroyer.destroy(options).await
            }
            BraavosCommands::List => {
                let lister = BraavosLister::new(self.working_directory.clone());
                lister.list()
            }
            BraavosCommands::Current => {
                let current = BraavosCurrent::new(self.working_directory.clone());
                current.current()
            }
        }
    }

    async fn handle_stark_command(&self, command: StarkCommands) -> Result<()> {
        match command {
            StarkCommands::Generate { env_file, file } => {
                let options = GenerateOptions {
                    env_file,
                    output_file: file,
                    use_arya_output: file.is_none(),
                };

                let generator = StarkGenerator::new(self.working_directory.clone());
                generator.generate(options).await
            }
        }
    }

    fn get_config_data(&self, config: Option<String>, config_file: Option<PathBuf>) -> Result<String> {
        if let Some(config_data) = config {
            Ok(config_data)
        } else if let Some(config_path) = config_file {
            std::fs::read_to_string(config_path)
                .map_err(|e| EnvieError::FileSystemError(format!("Failed to read config file: {}", e)))
        } else {
            // Try to read default config file
            let default_config = self.working_directory.join(".tf.layers.json");
            if default_config.exists() {
                std::fs::read_to_string(default_config)
                    .map_err(|e| EnvieError::FileSystemError(format!("Failed to read .tf.layers.json: {}", e)))
            } else {
                Err(EnvieError::ConfigError(
                    "--config parameter, --config-file parameter, or a .tf.layers.json file is required".to_string()
                ))
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

    async fn list_arya_environments(&self) -> Result<()> {
        let arya_dir = self.working_directory.join("cli").join(".arya");
        let terraform_manager = TerraformManager::new(&arya_dir);
        
        let workspaces = terraform_manager.workspace_list()?;
        let dev_workspaces: Vec<String> = workspaces
            .into_iter()
            .filter(|w| w != "default")
            .collect();

        let output_manager = OutputManager::new();
        
        if dev_workspaces.is_empty() {
            output_manager.print_yellow("No development environments available.");
        } else {
            output_manager.print_green("Available development environments:");
            for workspace in dev_workspaces {
                output_manager.print_blue(&workspace);
            }
        }

        Ok(())
    }

    async fn current_arya_environment(&self) -> Result<()> {
        let arya_dir = self.working_directory.join("cli").join(".arya");
        let terraform_manager = TerraformManager::new(&arya_dir);
        
        let workspace = terraform_manager.workspace_show()?;
        let output_manager = OutputManager::new();
        
        if workspace == "default" {
            output_manager.print_yellow("No development environment is currently active.");
        } else {
            output_manager.print_green("Current development environment:");
            output_manager.print_blue(&format!("* {}", workspace));
        }

        Ok(())
    }
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
