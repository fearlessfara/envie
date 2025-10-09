use crate::common::*;
use std::path::PathBuf;
use std::collections::HashMap;

pub struct ListCommand {
    working_directory: PathBuf,
    output_manager: OutputManager,
}

impl ListCommand {
    pub fn new(working_directory: PathBuf) -> Self {
        Self {
            working_directory,
            output_manager: OutputManager::new(),
        }
    }

    pub fn list(&self) -> Result<()> {
        // Find the project root
        let project_root = self.find_project_root()?;
        
        println!("📋 Listing all discovered units and their workspaces...\n");
        
        // Discover all units
        let mut discovery = UnitDiscovery::new(project_root.clone());
        discovery.discover_all()?;
        
        if discovery.registry.units.is_empty() {
            self.output_manager.print_yellow("No units found. Make sure you have envie.yaml files in your project.");
            return Ok(());
        }
        
        // Group workspaces by unit
        let mut unit_workspaces: HashMap<String, Vec<String>> = HashMap::new();

        for unit in discovery.registry.get_all_units() {
            let unit_path = project_root.join(&unit.path);

            // Try to get workspaces for this unit
            let terraform_manager = TerraformManager::new(&unit_path);
            if terraform_manager.init().is_ok() {
                if let Ok(workspaces) = terraform_manager.workspace_list() {
                    let dev_workspaces: Vec<String> = workspaces
                        .into_iter()
                        .filter(|w| w != "default" && w != "* default")
                        .collect();

                    if !dev_workspaces.is_empty() {
                        unit_workspaces.insert(unit.config.name.clone(), dev_workspaces);
                    }
                }
            }
        }
        
        if unit_workspaces.is_empty() {
            self.output_manager.print_yellow("No active workspaces found for any units.");
            println!("\nDiscovered units:");
            for unit in discovery.registry.get_all_units() {
                println!("  • {} ({:?}) - {}", unit.config.name, unit.config.unit_type, unit.path.display());
            }
        } else {
            self.output_manager.print_green("Active workspaces by unit:\n");
            for (unit_name, workspaces) in unit_workspaces {
                if let Some(unit) = discovery.registry.get_unit(&unit_name) {
                    println!("📦 {} ({:?})", unit_name, unit.config.unit_type);
                    println!("   Path: {}", unit.path.display());
                    println!("   Workspaces:");
                    for workspace in workspaces {
                        println!("     • {}", workspace);
                    }
                    println!();
                }
            }
        }

        Ok(())
    }
    
    fn find_project_root(&self) -> Result<PathBuf> {
        let mut current = self.working_directory.clone();
        
        loop {
            let workspace_file = current.join("workspace.envie.yaml");
            if workspace_file.exists() {
                return Ok(current);
            }
            
            if let Some(parent) = current.parent() {
                current = parent.to_path_buf();
            } else {
                return Ok(self.working_directory.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_list_command_creation() {
        let temp_dir = TempDir::new().unwrap();
        let lister = ListCommand::new(temp_dir.path().to_path_buf());
        assert_eq!(lister.working_directory, temp_dir.path());
    }
}
