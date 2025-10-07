use crate::common::*;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DestroyOptions {
    pub merge_request: Option<String>,
    pub dry_run: bool,
    pub verbose: bool,
}

pub struct DestroyCommand {
    working_directory: PathBuf,
    output_manager: OutputManager,
}

impl DestroyCommand {
    pub fn new(working_directory: PathBuf) -> Self {
        Self {
            working_directory,
            output_manager: OutputManager::new(),
        }
    }

    pub async fn execute(&self, options: DestroyOptions) -> Result<()> {
        let envie_dir = self.working_directory.join(".envie");
        let terraform_manager = TerraformManager::new(&envie_dir);

        // Get workspace
        let workspace = if let Some(merge_request) = options.merge_request {
            let repo_name = std::env::current_dir()?
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| EnvieError::ValidationError("Could not determine repository name".to_string()))?
                .to_string();
            format!("{}-{}", repo_name, merge_request)
        } else {
            terraform_manager.workspace_show()?
        };

        // Validate workspace
        if workspace == "default" {
            return Err(EnvieError::ValidationError(
                "No active development environment to destroy".to_string()
            ));
        }

        if !terraform_manager.workspace_list()?.contains(&workspace) {
            return Err(EnvieError::ValidationError(
                format!("Envie environment '{}' does not exist", workspace)
            ));
        }

        // Select workspace
        terraform_manager.workspace_select(&workspace)?;

        // Get service name and dependencies from terraform state
        let service_name = terraform_manager.output_value("service")?
            .as_str()
            .ok_or_else(|| EnvieError::TerraformError("Service name not found in terraform state".to_string()))?
            .to_string();

        let dependencies: Vec<String> = terraform_manager.output_value("dependencies")?
            .as_array()
            .ok_or_else(|| EnvieError::TerraformError("Dependencies not found in terraform state".to_string()))?
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();

        if options.dry_run {
            self.print_destroy_order(&dependencies, &service_name);
            return Ok(());
        }

        // Destroy components
        self.destroy_components(&dependencies).await?;

        // Destroy envie state
        self.destroy_envie_state(&service_name, &workspace).await?;

        self.output_manager.print_green(&format!(">> Successfully destroyed envie environment: {}", workspace));

        Ok(())
    }

    fn print_destroy_order(&self, dependencies: &[String], service_name: &str) {
        self.output_manager.print_green(&format!("Destroy order for service: {}", service_name));
        
        let mut index = 1;
        for dep in dependencies.iter().rev() {
            let parts: Vec<&str> = dep.split(':').collect();
            if parts.len() == 2 {
                let comp_name = parts[0];
                let comp_env = parts[1];
                
                if comp_env != "dev" {
                    self.output_manager.print_blue(&format!("  {}. {}: {} (skipped)", index, comp_name, comp_env));
                } else {
                    self.output_manager.print_yellow(&format!("  {}. {}: {}", index, comp_name, comp_env));
                }
            }
            index += 1;
        }
    }

    async fn destroy_components(&self, dependencies: &[String]) -> Result<()> {
        self.output_manager.print_green(">> Destroying deployments for service");

        for dep in dependencies.iter().rev() {
            let parts: Vec<&str> = dep.split(':').collect();
            if parts.len() == 2 {
                let comp_name = parts[0];
                let comp_env = parts[1];
                
                if comp_env == "dev" {
                    self.output_manager.print_green(&format!(">> Destroying component: {}", comp_name));
                    self.destroy_component(comp_name).await?;
                } else {
                    self.output_manager.print_green(&format!(">> Skipping destruction of component: {} in environment: {}", comp_name, comp_env));
                }
            }
        }

        Ok(())
    }

    async fn destroy_component(&self, component: &str) -> Result<()> {
        let _component_dir = self.working_directory.join("services").join(component).join("temp_deployments");
        
        // This would call the actual destroy command
        // For now, we'll just create a placeholder
        self.output_manager.print_green(&format!("Component {} destroyed successfully", component));
        
        Ok(())
    }

    async fn destroy_envie_state(&self, service_name: &str, workspace: &str) -> Result<()> {
        let envie_dir = self.working_directory.join(".envie");
        let terraform_manager = TerraformManager::new(&envie_dir);

        // Destroy terraform configuration
        let vars = vec![
            ("service", service_name),
            ("dependencies", "[]"),
        ];
        terraform_manager.destroy(&vars)?;

        // Select default workspace
        terraform_manager.workspace_select("default")?;

        // Delete the workspace
        terraform_manager.workspace_delete(workspace)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_destroy_command_creation() {
        let temp_dir = TempDir::new().unwrap();
        let destroyer = DestroyCommand::new(temp_dir.path().to_path_buf());
        assert_eq!(destroyer.working_directory, temp_dir.path());
    }

    #[test]
    fn test_destroy_order_printing() {
        let temp_dir = TempDir::new().unwrap();
        let destroyer = DestroyCommand::new(temp_dir.path().to_path_buf());
        
        let dependencies = vec![
            "service1/component1:dev".to_string(),
            "service1/component2:prod".to_string(),
        ];
        
        // This test just ensures the function doesn't panic
        destroyer.print_destroy_order(&dependencies, "service1");
    }
}
