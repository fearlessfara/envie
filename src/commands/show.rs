use crate::common::*;
use crate::common::service_config::WorkspaceConfig;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ShowOptions {
    pub service: Option<String>,
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

        // Discover all units using the new flexible system
        let mut discovery = UnitDiscovery::new(self.working_directory.clone());
        discovery.discover_all()?;
        
        if let Some(unit_name) = &options.service {
            // Show specific unit
            self.show_unit(unit_name, &discovery, &options)?;
        } else {
            // Show all units
            self.show_all_units(&discovery, &options)?;
        }

        Ok(())
    }

    fn show_all_units(&self, discovery: &UnitDiscovery, _options: &ShowOptions) -> Result<()> {
        self.output_manager.print_green("📋 Envie Project Overview");
        println!();

        // Show project info if workspace.envie exists
        if let Ok(workspace_config) = self.load_workspace_config() {
            if let Some(project) = &workspace_config.project {
                self.output_manager.print_blue("Project:");
                println!("  Name: {}", project.name);
                println!("  Description: {}", project.description);
                println!();
            }
        }

        // Show all discovered units grouped by type
        self.output_manager.print_blue("Discovered Units:");
        println!();
        
        // Group by unit type
        for unit_type in [UnitType::Layer, UnitType::Application, UnitType::Service, UnitType::Component, UnitType::Module] {
            let units = discovery.get_units_by_type(&unit_type);
            if !units.is_empty() {
                println!("  {:?}:", unit_type);
                for unit in units {
                    let indent = "  ".repeat(unit.level + 2);
                    println!("{}📦 {} - {}", indent, unit.config.name, unit.config.description);
                    println!("{}   Path: {}", indent, unit.path.display());
                    println!("{}   State: {:?}", indent, unit.config.state_management);
                    
                    if !unit.config.depends.is_empty() {
                        println!("{}   Dependencies:", indent);
                        for dep in &unit.config.depends {
                            println!("{}     - {} ({})", indent, dep.path, dep.environment);
                        }
                    }
                    println!();
                }
            }
        }

        Ok(())
    }

    fn show_unit(&self, unit_name: &str, discovery: &UnitDiscovery, _options: &ShowOptions) -> Result<()> {
        // Find the unit
        let unit = discovery.registry.get_unit(unit_name)
            .ok_or_else(|| EnvieError::ValidationError(
                format!("Unit '{}' not found", unit_name)
            ))?;

        self.output_manager.print_green(&format!("📦 Unit: {}", unit_name));
        println!();

        println!("  Type: {:?}", unit.config.unit_type);
        println!("  Description: {}", unit.config.description);
        println!("  Path: {}", unit.path.display());
        println!("  Level: {} (depth in structure)", unit.level);
        println!("  State Management: {:?}", unit.config.state_management);
        println!();

        if !unit.config.depends.is_empty() {
            self.output_manager.print_blue("  Dependencies:");
            for dep in &unit.config.depends {
                println!("    📎 {} ({})", dep.path, dep.environment);
            }
            println!();
        }

        if !unit.children.is_empty() {
            self.output_manager.print_blue("  Child Units:");
            for child_path in &unit.children {
                if let Some(child) = discovery.registry.get_unit_by_path(child_path) {
                    println!("    - {}", child.config.name);
                }
            }
            println!();
        }

        if let Some(parent_path) = &unit.parent {
            if let Some(parent) = discovery.registry.get_unit_by_path(parent_path) {
                self.output_manager.print_blue("  Parent Unit:");
                println!("    - {}", parent.config.name);
                println!();
            }
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
            verbose: true,
        };

        assert_eq!(options.service, Some("test-service".to_string()));
        assert!(options.verbose);
    }
}
