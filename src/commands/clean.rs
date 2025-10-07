use crate::common::*;
use std::path::PathBuf;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct CleanOptions {
    pub service_name: Option<String>,
    pub upgrade: bool,
    pub verbose: bool,
}

pub struct CleanCommand {
    working_directory: PathBuf,
    output_manager: OutputManager,
}

impl CleanCommand {
    pub fn new(working_directory: PathBuf) -> Self {
        Self {
            working_directory,
            output_manager: OutputManager::new(),
        }
    }

    pub fn execute(&self, options: CleanOptions) -> Result<()> {
        let services_dir = if let Some(service_name) = &options.service_name {
            self.working_directory.join("services").join(service_name)
        } else {
            self.working_directory.join("services")
        };

        if options.service_name.is_some() {
            self.output_manager.print_blue(&format!("Cleaning service: {}", options.service_name.as_ref().unwrap()));
        } else {
            self.output_manager.print_blue("Cleaning all services");
        }

        // Clean .terraform directories
        self.clean_terraform_directories(&services_dir)?;

        // Initialize terraform in main and temp_deployments directories
        self.initialize_terraform_directories(&services_dir, options.upgrade)?;

        // Clean and initialize .envie directory
        self.clean_envie_directory(options.upgrade)?;

        self.output_manager.print_green("Terraform initialization and workspace selection complete in specified services.");

        Ok(())
    }

    fn clean_terraform_directories(&self, services_dir: &std::path::Path) -> Result<()> {
        // Find and delete all .terraform directories, excluding stable_deployments
        let entries: Vec<_> = WalkDir::new(services_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name() == ".terraform" 
                    && e.file_type().is_dir()
                    && !e.path().to_string_lossy().contains("stable_deployments")
            })
            .collect();

        for entry in entries {
            if let Err(e) = std::fs::remove_dir_all(entry.path()) {
                log::warn!("Failed to remove .terraform directory {}: {}", entry.path().display(), e);
            }
        }

        self.output_manager.print_green("Deleted all .terraform directories in specified services, excluding stable_deployments.");
        Ok(())
    }

    fn initialize_terraform_directories(&self, services_dir: &std::path::Path, upgrade: bool) -> Result<()> {
        // Find all main and temp_deployments directories
        let main_dirs: Vec<_> = WalkDir::new(services_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name() == "main" && e.file_type().is_dir())
            .map(|e| e.path().to_path_buf())
            .collect();

        let temp_deployment_dirs: Vec<_> = WalkDir::new(services_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name() == "temp_deployments" && e.file_type().is_dir())
            .map(|e| e.path().to_path_buf())
            .collect();

        let all_dirs: Vec<_> = main_dirs.into_iter().chain(temp_deployment_dirs).collect();

        // Initialize terraform in each directory
        for dir in all_dirs {
            self.output_manager.print_blue(&format!("Initializing Terraform in {}", dir.display()));
            
            let terraform_manager = TerraformManager::new(&dir);
            
            if upgrade {
                terraform_manager.init_with_upgrade()?;
            } else {
                terraform_manager.init()?;
            }

            terraform_manager.workspace_select("default")?;
        }

        Ok(())
    }

    fn clean_envie_directory(&self, upgrade: bool) -> Result<()> {
        let envie_dir = self.working_directory.join(".envie");
        
        self.output_manager.print_blue("Cleaning .terraform directory in .envie");
        
        // Remove .terraform directory
        let terraform_dir = envie_dir.join(".terraform");
        if terraform_dir.exists() {
            std::fs::remove_dir_all(&terraform_dir)?;
        }

        // Initialize terraform
        let terraform_manager = TerraformManager::new(&envie_dir);
        
        if upgrade {
            terraform_manager.init_with_upgrade()?;
        } else {
            terraform_manager.init()?;
        }
        
        terraform_manager.workspace_select("default")?;
        
        self.output_manager.print_green("Terraform initialization and workspace selection complete in .envie.");
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_clean_command_creation() {
        let temp_dir = TempDir::new().unwrap();
        let cleaner = CleanCommand::new(temp_dir.path().to_path_buf());
        assert_eq!(cleaner.working_directory, temp_dir.path());
    }

    #[test]
    fn test_clean_options() {
        let options = CleanOptions {
            service_name: Some("test-service".to_string()),
            upgrade: true,
            verbose: false,
        };
        
        assert_eq!(options.service_name, Some("test-service".to_string()));
        assert!(options.upgrade);
        assert!(!options.verbose);
    }
}
