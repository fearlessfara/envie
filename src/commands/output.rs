use crate::common::*;
use crate::common::service_config::WorkspaceConfig;
use std::path::PathBuf;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct OutputOptions {
    pub output_file: Option<String>,
    pub env_id: String,
    pub unit_name: Option<String>,
    pub format: OutputFormat,
}

#[derive(Debug, Clone)]
pub enum OutputFormat {
    Json,
    Table,
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
        // Find the project root
        let project_root = self.find_project_root()?;

        // Get project name and workspace
        let project_name = self.get_project_name()?;
        let workspace = format!("{}-{}", project_name, options.env_id);

        // Discover all units
        let mut discovery = UnitDiscovery::new(project_root.clone());
        discovery.discover_all()?;

        if discovery.registry.units.is_empty() {
            return Err(EnvieError::ValidationError(
                "No deployable units found. Make sure you have envie.yaml files in your project.".to_string()
            ));
        }

        // Determine which units to get outputs from
        let units_to_query = if let Some(ref unit_name) = options.unit_name {
            // Query specific unit
            let matches = discovery.registry.resolve_unit(unit_name);
            if matches.is_empty() {
                return Err(EnvieError::ValidationError(
                    format!("Unit '{}' not found", unit_name)
                ));
            }
            matches
        } else {
            // Query all units
            discovery.registry.get_all_units()
        };

        // Collect outputs from all units
        let mut all_outputs: HashMap<String, serde_json::Value> = HashMap::new();

        for unit in &units_to_query {
            let unit_path = project_root.join(&unit.path);

            match self.get_unit_outputs(&unit_path, &workspace, &unit.config.name).await {
                Ok(outputs) => {
                    // Prefix outputs with unit name to avoid conflicts
                    for (key, value) in outputs {
                        let prefixed_key = format!("{}_{}", unit.config.name, key);
                        all_outputs.insert(prefixed_key, value);
                    }
                }
                Err(e) => {
                    eprintln!("⚠️  Warning: Failed to get outputs from unit '{}': {}", unit.config.name, e);
                }
            }
        }

        // Format and display outputs
        match options.format {
            OutputFormat::Json => {
                let json_output = serde_json::to_string_pretty(&all_outputs)?;

                if let Some(output_file) = options.output_file {
                    std::fs::write(&output_file, &json_output)?;
                    self.output_manager.print_green(&format!("✅ Outputs saved to {}", output_file));
                } else {
                    println!("{}", json_output);
                }
            }
            OutputFormat::Table => {
                self.print_outputs_table(&all_outputs, &workspace)?;
            }
        }

        Ok(())
    }

    async fn get_unit_outputs(&self, unit_path: &PathBuf, workspace: &str, unit_name: &str) -> Result<HashMap<String, serde_json::Value>> {
        let terraform_manager = TerraformManager::new(unit_path);

        // Check if terraform is initialized
        let terraform_dir = unit_path.join(".terraform");
        if !terraform_dir.exists() {
            return Err(EnvieError::TerraformError(
                format!("Terraform not initialized in unit '{}'", unit_name)
            ));
        }

        // Select the workspace
        let workspaces = terraform_manager.workspace_list()?;
        if !workspaces.iter().any(|w| w == workspace) {
            return Err(EnvieError::TerraformError(
                format!("Workspace '{}' does not exist for unit '{}'", workspace, unit_name)
            ));
        }
        terraform_manager.workspace_select(workspace)?;

        // Get terraform outputs
        let outputs = terraform_manager.output_json()?;

        // Convert to HashMap<String, serde_json::Value>
        let mut result = HashMap::new();
        for (key, output) in outputs {
            result.insert(key, output.value);
        }

        Ok(result)
    }

    fn print_outputs_table(&self, outputs: &HashMap<String, serde_json::Value>, workspace: &str) -> Result<()> {
        self.output_manager.print_green(&format!("\n📊 Terraform Outputs for workspace: {}\n", workspace));

        if outputs.is_empty() {
            println!("  (No outputs found)");
            return Ok(());
        }

        // Group outputs by unit
        let mut outputs_by_unit: HashMap<String, Vec<(String, serde_json::Value)>> = HashMap::new();

        for (key, value) in outputs {
            if let Some(separator_pos) = key.find('_') {
                let unit_name = &key[..separator_pos];
                let output_key = &key[separator_pos + 1..];

                outputs_by_unit
                    .entry(unit_name.to_string())
                    .or_insert_with(Vec::new)
                    .push((output_key.to_string(), value.clone()));
            } else {
                outputs_by_unit
                    .entry("unknown".to_string())
                    .or_insert_with(Vec::new)
                    .push((key.clone(), value.clone()));
            }
        }

        // Print outputs grouped by unit
        for (unit_name, unit_outputs) in outputs_by_unit.iter() {
            println!("┌─ {} ─────────────────────────────────", unit_name);

            for (key, value) in unit_outputs {
                let value_str = match value {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                        serde_json::to_string_pretty(value).unwrap_or_else(|_| "...".to_string())
                    }
                    serde_json::Value::Null => "null".to_string(),
                };

                println!("│  {}: {}", key, value_str);
            }
            println!("└────────────────────────────────────────\n");
        }

        Ok(())
    }

    fn get_project_name(&self) -> Result<String> {
        let workspace_file = self.working_directory.join("workspace.envie.yaml");
        if !workspace_file.exists() {
            return Ok("envie-project".to_string());
        }

        let content = std::fs::read_to_string(&workspace_file)?;
        let config: WorkspaceConfig = serde_yaml::from_str(&content)?;

        Ok(config.project.as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "envie-project".to_string()))
    }

    fn find_project_root(&self) -> Result<PathBuf> {
        let mut current = self.working_directory.clone();

        loop {
            let workspace_file = current.join("workspace.envie.yaml");
            if workspace_file.exists() {
                return Ok(current);
            }

            // Move up one directory
            if let Some(parent) = current.parent() {
                current = parent.to_path_buf();
            } else {
                // No workspace.envie.yaml found, use working directory as fallback
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
