use crate::common::*;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct OutputOptions {
    pub output_file: Option<String>,
    pub verbose: bool,
}

pub struct OutputCommand {
    working_directory: PathBuf,
    output_manager: OutputManager,
}

impl OutputCommand {
    pub fn new(working_directory: PathBuf) -> Self {
        Self {
            working_directory,
            output_manager: OutputManager::new(),
        }
    }

    pub async fn execute(&self, options: OutputOptions) -> Result<()> {
        let envie_dir = self.working_directory.join(".envie");
        let terraform_manager = TerraformManager::new(&envie_dir);

        // Get current workspace
        let workspace = terraform_manager.workspace_show()?;
        terraform_manager.workspace_select(&workspace)?;

        // Get service name and dependencies
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

        // Get combined outputs
        let combined_output = self.get_combined_output(&dependencies).await?;

        // Print or save output
        if let Some(output_file) = options.output_file {
            let full_path = std::fs::canonicalize(&output_file)
                .unwrap_or_else(|_| PathBuf::from(&output_file));
            std::fs::write(&full_path, serde_json::to_string_pretty(&combined_output)?)?;
            self.output_manager.print_green(&format!("Terraform outputs saved to {}", full_path.display()));
        } else {
            self.output_manager.print_blue(&format!("Combined Terraform outputs for service: {}", service_name));
            println!("{}", serde_json::to_string_pretty(&combined_output)?);
        }

        Ok(())
    }

    async fn get_combined_output(&self, dependencies: &[String]) -> Result<serde_json::Value> {
        let mut combined_outputs = serde_json::Map::new();

        // Separate dev and non-dev components
        let (dev_components, non_dev_components): (Vec<_>, Vec<_>) = dependencies
            .iter()
            .partition(|dep| dep.ends_with(":dev"));

        // Process non-dev components (stable deployments)
        let mut unique_service_envs = std::collections::HashSet::new();
        for comp in &non_dev_components {
            let parts: Vec<&str> = comp.split(':').collect();
            if parts.len() == 2 {
                let comp_name = parts[0];
                let comp_env = parts[1];
                let service_name = comp_name.split('/').next().unwrap();
                unique_service_envs.insert((service_name, comp_env));
            }
        }

        // Get outputs for stable deployments
        for (service, env) in unique_service_envs {
            let service_dir = self.working_directory.join("services").join(service).join("stable_deployments");
            if service_dir.exists() {
                let output = self.get_terraform_output(&service_dir, env).await?;
                self.merge_outputs(&mut combined_outputs, output);
            }
        }

        // Get outputs for dev components (temp deployments)
        for comp in &dev_components {
            let parts: Vec<&str> = comp.split(':').collect();
            if parts.len() == 2 {
                let comp_name = parts[0];
                let comp_env = parts[1];
                let component_dir = self.working_directory.join("services").join(comp_name).join("temp_deployments");
                if component_dir.exists() {
                    let output = self.get_terraform_output(&component_dir, comp_env).await?;
                    self.merge_outputs(&mut combined_outputs, output);
                }
            }
        }

        Ok(serde_json::Value::Object(combined_outputs))
    }

    async fn get_terraform_output(&self, dir: &std::path::Path, env: &str) -> Result<serde_json::Value> {
        let terraform_manager = TerraformManager::new(dir);

        // Initialize terraform if not dev environment
        if env != "dev" {
            let backend_config = dir.join("backend").join(format!("{}.conf", env));
            if backend_config.exists() {
                // This would run terraform init with backend config
                // For now, we'll skip this step
            }
        }

        // Check if terraform is initialized
        let terraform_dir = dir.join(".terraform");
        if !terraform_dir.exists() {
            return Err(EnvieError::TerraformError(
                format!("Terraform not initialized in {}", dir.display())
            ));
        }

        // Get terraform outputs
        let outputs = terraform_manager.output_json()?;
        
        // Convert to the expected format
        let mut result = serde_json::Map::new();
        for (key, output) in outputs {
            result.insert(key, output.value);
        }

        Ok(serde_json::Value::Object(result))
    }

    fn merge_outputs(&self, combined: &mut serde_json::Map<String, serde_json::Value>, new: serde_json::Value) {
        if let serde_json::Value::Object(new_map) = new {
            for (key, value) in new_map {
                combined.insert(key, value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_output_command_creation() {
        let temp_dir = TempDir::new().unwrap();
        let output = OutputCommand::new(temp_dir.path().to_path_buf());
        assert_eq!(output.working_directory, temp_dir.path());
    }

    #[test]
    fn test_merge_outputs() {
        let temp_dir = TempDir::new().unwrap();
        let output = OutputCommand::new(temp_dir.path().to_path_buf());
        
        let mut combined = serde_json::Map::new();
        let new = serde_json::json!({
            "key1": "value1",
            "key2": "value2"
        });
        
        output.merge_outputs(&mut combined, new);
        
        assert_eq!(combined.len(), 2);
        assert_eq!(combined.get("key1").unwrap(), "value1");
        assert_eq!(combined.get("key2").unwrap(), "value2");
    }
}
