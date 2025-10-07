use crate::common::*;
use std::path::PathBuf;

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
        let envie_dir = self.working_directory.join(".envie");
        let terraform_manager = TerraformManager::new(&envie_dir);
        
        let workspaces = terraform_manager.workspace_list()?;
        let dev_workspaces: Vec<String> = workspaces
            .into_iter()
            .filter(|w| w != "default")
            .collect();

        if dev_workspaces.is_empty() {
            self.output_manager.print_yellow("No development environments available.");
        } else {
            self.output_manager.print_green("Available development environments:");
            for workspace in dev_workspaces {
                self.output_manager.print_blue(&workspace);
            }
        }

        Ok(())
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
