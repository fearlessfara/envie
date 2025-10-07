use crate::common::*;
use crate::common::environment::{EnvironmentConfig, EphemeralConfig, BackendConfig as EnvironmentBackendConfig};
use crate::common::service_config::WorkspaceConfig;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DeployV2Options {
    pub service_name: Option<String>,
    pub merge_request: String,
    pub environment_overrides: HashMap<String, String>,
    pub dry_run: bool,
    pub no_prompt: bool,
    pub verbose: bool,
}

pub struct DeployV2Command {
    working_directory: PathBuf,
    output_manager: OutputManager,
}

impl DeployV2Command {
    pub fn new(working_directory: PathBuf) -> Self {
        Self {
            working_directory,
            output_manager: OutputManager::new(),
        }
    }
    
    pub async fn execute(&self, options: DeployV2Options) -> Result<()> {
        // Environment overrides are already parsed by the CLI handler
        let environment_overrides = &options.environment_overrides;
        // Discover services from current directory
        let registry = ServiceRegistry::discover_from_path(&self.working_directory)?;
        
        if registry.services.is_empty() {
            return Err(EnvieError::ValidationError(
                "No services found. Make sure you're in a directory with .envie files or run from the project root.".to_string()
            ));
        }
        
        // Determine which service(s) to deploy
        let services_to_deploy = if let Some(service_name) = options.service_name {
            if let Some(service) = registry.services.get(&service_name) {
                vec![service.clone()]
            } else {
                return Err(EnvieError::ValidationError(
                    format!("Service '{}' not found", service_name)
                ));
            }
        } else {
            // Deploy the service in the current directory
            if let Some(service) = registry.find_service_by_path(&self.working_directory) {
                vec![service.clone()]
            } else {
                return Err(EnvieError::ValidationError(
                    "No service found in current directory. Specify a service name or run from a service directory.".to_string()
                ));
            }
        };
        
        // Resolve workspace name
        let project_name = self.get_project_name()?;
        let workspace = format!("{}-{}", project_name, options.merge_request);
        
        // Load environment configuration
        let environment_config = self.load_environment_config()?;
        
        // Create environment resolver
        let environment_resolver = EnvironmentResolver::new(
            workspace.clone(),
            project_name,
            environment_config,
        ).with_available_workspaces(self.get_available_workspaces()?);
        
        // Deploy each service
        for service in services_to_deploy {
            self.deploy_service(&service, &workspace, &environment_resolver, &environment_overrides, options.dry_run).await?;
        }
        
        Ok(())
    }
    
    async fn deploy_service(
        &self,
        service: &DiscoveredService,
        workspace: &str,
        environment_resolver: &EnvironmentResolver,
        environment_overrides: &HashMap<String, String>,
        dry_run: bool,
    ) -> Result<()> {
        self.output_manager.print_green(&format!("Deploying service: {}", service.config.name));
        
        // Resolve dependencies
        let registry = ServiceRegistry::discover_from_path(&self.working_directory)?;
        let deployment_order = registry.resolve_dependencies(&service.config.name)?;
        
        if dry_run {
            self.print_deployment_plan(&deployment_order, environment_resolver)?;
            return Ok(());
        }
        
        // Deploy modules in dependency order
        for module in &service.modules {
            self.deploy_module(module, workspace, environment_resolver, environment_overrides, &service.config.name).await?;
        }
        
        Ok(())
    }
    
    async fn deploy_module(
        &self,
        module: &DiscoveredModule,
        workspace: &str,
        environment_resolver: &EnvironmentResolver,
        environment_overrides: &HashMap<String, String>,
        service_name: &str,
    ) -> Result<()> {
        self.output_manager.print_green(&format!("  Deploying module: {}", module.config.name));
        
        // Generate Terraform files
        let generator = TerraformGenerator::new();
        generator.write_generated_files(
            &module.path,
            &module.config.depends,
            &module.config,
            environment_resolver,
            environment_overrides,
            service_name,
            &module.config.name,
        )?;
        
        // Initialize and apply Terraform
        let terraform_manager = TerraformManager::new(&module.path);
        terraform_manager.init()?;
        
        // Create or select workspace
        if terraform_manager.workspace_list()?.iter().any(|w| w == workspace) {
            terraform_manager.workspace_select(workspace)?;
        } else {
            terraform_manager.workspace_new(workspace)?;
        }
        
        // Apply Terraform
        terraform_manager.apply(&[])?;
        
        self.output_manager.print_green(&format!("  ✓ Module {} deployed successfully", module.config.name));
        
        Ok(())
    }
    
    fn print_deployment_plan(
        &self,
        deployment_order: &[String],
        _environment_resolver: &EnvironmentResolver,
    ) -> Result<()> {
        self.output_manager.print_yellow("Deployment Plan:");
        
        for (i, service_name) in deployment_order.iter().enumerate() {
            self.output_manager.print_yellow(&format!("  {}. {}", i + 1, service_name));
        }
        
        self.output_manager.print_yellow("\nRemote State Dependencies:");
        
        // This would need to be implemented to show what remote states will be referenced
        // For now, just show a placeholder
        self.output_manager.print_yellow("  (Remote state dependencies will be shown here)");
        
        Ok(())
    }
    
    fn load_environment_config(&self) -> Result<EnvironmentConfig> {
        // Try to load workspace.envie first
        let workspace_envie = self.working_directory.join("workspace.envie");
        if workspace_envie.exists() {
            let workspace_config = WorkspaceConfig::from_file(workspace_envie)?;
            return Ok(EnvironmentConfig {
                project: workspace_config.project,
                ephemeral: EphemeralConfig {
                    naming_pattern: "{project}-{id}".to_string(),
                    backend: EnvironmentBackendConfig {
                        backend_type: "s3".to_string(),
                        config: {
                            let mut config = std::collections::HashMap::new();
                            config.insert("bucket".to_string(), "terraform-state-ephemeral".to_string());
                            config.insert("region".to_string(), "eu-west-1".to_string());
                            config
                        },
                    },
                },
                stable: std::collections::HashMap::new(),
            });
        }
        
        // Fallback to default configuration
        Ok(EnvironmentConfig {
            project: None,
            ephemeral: EphemeralConfig {
                naming_pattern: "{project}-{id}".to_string(),
                backend: EnvironmentBackendConfig {
                    backend_type: "s3".to_string(),
                    config: {
                        let mut config = std::collections::HashMap::new();
                        config.insert("bucket".to_string(), "terraform-state-ephemeral".to_string());
                        config.insert("region".to_string(), "eu-west-1".to_string());
                        config
                    },
                },
            },
            stable: std::collections::HashMap::new(),
        })
    }
    
    fn get_available_workspaces(&self) -> Result<Vec<String>> {
        // This would typically query Terraform workspaces or S3 buckets
        // For now, return a placeholder
        Ok(vec!["myapp-123".to_string(), "myapp-456".to_string()])
    }
    
    fn get_project_name(&self) -> Result<String> {
        // Try to load from workspace config first
        let workspace_envie = self.working_directory.join("workspace.envie");
        if workspace_envie.exists() {
            if let Ok(config) = EnvironmentConfig::from_file(workspace_envie) {
                if let Some(project) = &config.project {
                    return Ok(project.name.clone());
                }
            }
        }
        
        // Fallback to directory name
        std::env::current_dir()?
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| EnvieError::ValidationError("Could not determine project name".to_string()))?
            .to_string()
            .pipe(Ok)
    }
    
}

// Helper trait for method chaining
trait Pipe<T> {
    fn pipe<F, U>(self, f: F) -> U where F: FnOnce(T) -> U;
}

impl<T> Pipe<T> for T {
    fn pipe<F, U>(self, f: F) -> U where F: FnOnce(T) -> U {
        f(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_deploy_command_creation() {
        let temp_dir = TempDir::new().unwrap();
        let command = DeployV2Command::new(temp_dir.path().to_path_buf());
        assert_eq!(command.working_directory, temp_dir.path());
    }
}
