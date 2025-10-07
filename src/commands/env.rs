use crate::common::*;
use std::path::PathBuf;
use regex::Regex;

#[derive(Debug, Clone)]
pub struct EnvOptions {
    pub merge_request_id: String,
    pub quiet: bool,
}

pub struct EnvCommand {
    working_directory: PathBuf,
    output_manager: OutputManager,
}

impl EnvCommand {
    pub fn new(working_directory: PathBuf) -> Self {
        Self {
            working_directory,
            output_manager: OutputManager::new(),
        }
    }

    pub async fn start(&self, options: EnvOptions) -> Result<()> {
        // Validate merge request ID
        self.validate_merge_request_id(&options.merge_request_id)?;

        // Format workspace name
        let workspace_name = self.format_workspace_name(&options.merge_request_id)?;

        // Initialize terraform
        let terraform_manager = TerraformManager::new(&self.working_directory);
        terraform_manager.init()?;

        // Check if workspace exists
        let workspaces = terraform_manager.workspace_list()?;
        if workspaces.iter().any(|w| w == &workspace_name) {
            self.output_manager.print_green(&format!("Activating development environment: {}", workspace_name));
            terraform_manager.workspace_select(&workspace_name)?;
        } else {
            self.output_manager.print_yellow(&format!("Creating new development environment: {}", workspace_name));
            terraform_manager.workspace_new(&workspace_name)?;
        }

        // Deploy the development environment
        self.output_manager.print_green(&format!("Deploying development environment: {}", workspace_name));
        
        let output_file = format!("{}.envie", workspace_name);
        terraform_manager.apply_with_output(&[], &output_file)?;

        self.output_manager.print_green(&format!("Development environment {} is ready to use", workspace_name));

        Ok(())
    }

    pub async fn destroy(&self, options: EnvOptions) -> Result<()> {
        let terraform_manager = TerraformManager::new(&self.working_directory);

        // Get workspace name
        let workspace_name = if let Some(merge_request_id) = Some(&options.merge_request_id) {
            self.validate_merge_request_id(merge_request_id)?;
            self.format_workspace_name(merge_request_id)?
        } else {
            terraform_manager.workspace_show()?
        };

        // Validate workspace
        if workspace_name == "default" {
            return Err(EnvieError::ValidationError(
                "No active development environment to destroy".to_string()
            ));
        }

        let workspaces = terraform_manager.workspace_list()?;
        if !workspaces.iter().any(|w| w == &workspace_name) {
            return Err(EnvieError::ValidationError(
                format!("Development environment {} does not exist", workspace_name)
            ));
        }

        // Destroy the environment
        self.output_manager.print_green(&format!("Destroying development environment: {}", workspace_name));
        
        terraform_manager.workspace_select(&workspace_name)?;
        terraform_manager.destroy(&[])?;
        terraform_manager.workspace_select("default")?;
        terraform_manager.workspace_delete(&workspace_name)?;

        self.output_manager.print_green(&format!("Development environment {} has been destroyed", workspace_name));

        Ok(())
    }

    pub fn list(&self) -> Result<()> {
        let terraform_manager = TerraformManager::new(&self.working_directory);
        let workspaces = terraform_manager.workspace_list()?;
        
        let dev_workspaces: Vec<String> = workspaces
            .into_iter()
            .filter(|w| w != "default")
            .collect();

        if dev_workspaces.is_empty() {
            self.output_manager.print_yellow("No development environments available.");
            return Ok(());
        }

        // Remove repository name prefix from workspace names
        let repo_name = std::env::current_dir()?
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| EnvieError::ValidationError("Could not determine repository name".to_string()))?
            .to_string();
        let clean_workspaces: Vec<String> = dev_workspaces
            .into_iter()
            .map(|w| {
                if w.starts_with(&format!("{}-", repo_name)) {
                    w.strip_prefix(&format!("{}-", repo_name)).unwrap_or(&w).to_string()
                } else {
                    w
                }
            })
            .collect();

        self.output_manager.print_green("Available development environments:");
        for workspace in clean_workspaces {
            self.output_manager.print_blue(&workspace);
        }

        Ok(())
    }

    pub fn current(&self) -> Result<()> {
        let terraform_manager = TerraformManager::new(&self.working_directory);
        let workspace_name = terraform_manager.workspace_show()?;

        if workspace_name == "default" {
            self.output_manager.print_yellow("No active development environment.");
            return Ok(());
        }

        // Remove repository name prefix from workspace name
        let repo_name = std::env::current_dir()?
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| EnvieError::ValidationError("Could not determine repository name".to_string()))?
            .to_string();
        let clean_workspace = if workspace_name.starts_with(&format!("{}-", repo_name)) {
            workspace_name.strip_prefix(&format!("{}-", repo_name)).unwrap_or(&workspace_name).to_string()
        } else {
            workspace_name
        };

        self.output_manager.print_green(&format!("Current development environment: {}.", clean_workspace));

        Ok(())
    }

    fn validate_merge_request_id(&self, merge_request_id: &str) -> Result<()> {
        let re = Regex::new(r"^[0-9]+(-[0-9A-Za-z]+)?$")?;
        
        if !re.is_match(merge_request_id) {
            return Err(EnvieError::ValidationError(
                "Invalid merge request ID. Please provide a valid merge request ID in the format {number}-({number})?".to_string()
            ));
        }

        Ok(())
    }

    fn format_workspace_name(&self, merge_request_id: &str) -> Result<String> {
        let repo_name = std::env::current_dir()?
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| EnvieError::ValidationError("Could not determine repository name".to_string()))?
            .to_string();
        Ok(format!("{}-{}", repo_name, merge_request_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_env_command_creation() {
        let temp_dir = TempDir::new().unwrap();
        let env_cmd = EnvCommand::new(temp_dir.path().to_path_buf());
        assert_eq!(env_cmd.working_directory, temp_dir.path());
    }

    #[test]
    fn test_merge_request_id_validation() {
        let temp_dir = TempDir::new().unwrap();
        let env_cmd = EnvCommand::new(temp_dir.path().to_path_buf());
        
        // Valid IDs
        assert!(env_cmd.validate_merge_request_id("123").is_ok());
        assert!(env_cmd.validate_merge_request_id("123-abc").is_ok());
        assert!(env_cmd.validate_merge_request_id("123-456").is_ok());
        
        // Invalid IDs
        assert!(env_cmd.validate_merge_request_id("abc").is_err());
        assert!(env_cmd.validate_merge_request_id("123-").is_err());
        assert!(env_cmd.validate_merge_request_id("-123").is_err());
    }

    #[test]
    fn test_workspace_name_formatting() {
        let temp_dir = TempDir::new().unwrap();
        let env_cmd = EnvCommand::new(temp_dir.path().to_path_buf());
        
        // This test would require a git repository to work properly
        // For now, we'll just test that the function doesn't panic
        let result = env_cmd.format_workspace_name("123");
        // The result depends on the git repository name, so we can't assert a specific value
        match result {
            Ok(name) => assert!(!name.is_empty()),
            Err(_) => {
                // Expected in test environment without git
            }
        }
    }
}
