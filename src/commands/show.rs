use crate::common::*;
use crate::common::service_config::{ServiceConfig, WorkspaceConfig};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ShowOptions {
    pub service: Option<String>,
    pub modules: bool,
    pub dependencies: bool,
    pub verbose: bool,
}

pub struct ShowCommand {
    working_directory: PathBuf,
    output_manager: OutputManager,
}

impl ShowCommand {
    pub fn new(working_directory: PathBuf) -> Self {
        Self {
            working_directory,
            output_manager: OutputManager::new(),
        }
    }

    pub fn execute(&self, options: ShowOptions) -> Result<()> {
        if options.verbose {
            println!("🔍 Analyzing Envie project structure...");
        }

        // Load workspace configuration
        let workspace_config = self.load_workspace_config()?;
        
        if let Some(service_name) = &options.service {
            // Show specific service
            self.show_service(service_name, &options)?;
        } else {
            // Show all services
            self.show_all_services(&workspace_config, &options)?;
        }

        Ok(())
    }

    fn load_workspace_config(&self) -> Result<WorkspaceConfig> {
        let workspace_file = self.working_directory.join("workspace.envie");
        if !workspace_file.exists() {
            return Err(EnvieError::ValidationError(
                "No workspace.envie found. Run 'envie init' first.".to_string()
            ));
        }

        let content = std::fs::read_to_string(&workspace_file)?;
        let config: WorkspaceConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    fn show_all_services(&self, workspace_config: &WorkspaceConfig, options: &ShowOptions) -> Result<()> {
        self.output_manager.print_green("📋 Envie Project Overview");
        println!();

        // Show project info
        if let Some(project) = &workspace_config.project {
            self.output_manager.print_blue("Project:");
            println!("  Name: {}", project.name);
            println!("  Description: {}", project.description);
            println!();
        }

        // Show services
        self.output_manager.print_blue("Services:");
        for service_discovery in &workspace_config.services {
            let service_name = service_discovery.name.as_ref()
                .cloned()
                .unwrap_or_else(|| service_discovery.path.split('/').last().unwrap_or("unknown").to_string());
            
            println!("  📦 {}", service_name);
            
            // Load and show service details
            if let Ok(service_config) = self.load_service_config(&service_discovery.path) {
                if options.modules || (!options.dependencies && !options.modules) {
                    self.show_service_modules(&service_config, "    ");
                }
                if options.dependencies || (!options.dependencies && !options.modules) {
                    self.show_service_dependencies(&service_config, "    ");
                }
            }
            println!();
        }

        Ok(())
    }

    fn show_service(&self, service_name: &str, options: &ShowOptions) -> Result<()> {
        // Find the service in workspace config
        let workspace_config = self.load_workspace_config()?;
        let service_discovery = workspace_config.services
            .iter()
            .find(|s| s.name.as_ref().map(|n| n == service_name).unwrap_or(false) ||
                     s.path.split('/').last().unwrap_or("") == service_name)
            .ok_or_else(|| EnvieError::ValidationError(
                format!("Service '{}' not found", service_name)
            ))?;

        self.output_manager.print_green(&format!("📦 Service: {}", service_name));
        println!();

        // Load service configuration
        let service_config = self.load_service_config(&service_discovery.path)?;
        
        println!("  Description: {}", service_config.description);
        println!();

        if options.modules || (!options.dependencies && !options.modules) {
            self.show_service_modules(&service_config, "  ");
        }
        
        if options.dependencies || (!options.dependencies && !options.modules) {
            self.show_service_dependencies(&service_config, "  ");
        }

        Ok(())
    }

    fn load_service_config(&self, service_path: &str) -> Result<ServiceConfig> {
        let service_dir = self.working_directory.join(service_path);
        let envie_file = service_dir.join(".envie");
        
        if !envie_file.exists() {
            return Err(EnvieError::ValidationError(
                format!("No .envie file found in {}", service_path)
            ));
        }

        let content = std::fs::read_to_string(&envie_file)?;
        let config: ServiceConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    fn show_service_modules(&self, service_config: &ServiceConfig, indent: &str) {
        self.output_manager.print_blue(&format!("{}Modules:", indent));
        for module in &service_config.modules {
            println!("{}  🔧 {}", indent, module.name);
            println!("{}     Description: {}", indent, module.description);
            println!("{}     Path: {}", indent, module.path);
            
            if !module.depends.is_empty() {
                println!("{}     Dependencies:", indent);
                for dep in &module.depends {
                    println!("{}       - {} ({})", indent, dep.path, dep.environment);
                }
            }
            println!();
        }
    }

    fn show_service_dependencies(&self, service_config: &ServiceConfig, indent: &str) {
        if !service_config.depends.is_empty() {
            self.output_manager.print_blue(&format!("{}Service Dependencies:", indent));
            for dep in &service_config.depends {
                println!("{}  📎 {}", indent, dep);
            }
            println!();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_show_command_creation() {
        let temp_dir = TempDir::new().unwrap();
        let show_cmd = ShowCommand::new(temp_dir.path().to_path_buf());
        assert_eq!(show_cmd.working_directory, temp_dir.path());
    }

    #[test]
    fn test_show_options() {
        let options = ShowOptions {
            service: Some("test-service".to_string()),
            modules: true,
            dependencies: false,
            verbose: true,
        };
        
        assert_eq!(options.service, Some("test-service".to_string()));
        assert!(options.modules);
        assert!(!options.dependencies);
        assert!(options.verbose);
    }
}
