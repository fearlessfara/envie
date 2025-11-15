use crate::common::*;
use crate::common::environment::EnvironmentConfig;
use std::path::PathBuf;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct DestroyOptions {
    pub unit_name: Option<String>,
    pub env_id: Option<String>,
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
        // Find the project root
        let project_root = self.find_project_root()?;

        if options.verbose {
            println!("🗑️  Starting destroy with flexible unit discovery...");
            println!("📂 Project root: {}", project_root.display());
        }

        // Discover all units
        let mut discovery = UnitDiscovery::new(project_root.clone());
        discovery.discover_all()?;

        if discovery.registry.units.is_empty() {
            return Err(EnvieError::ValidationError(
                "No deployable units found. Make sure you have envie.yaml files in your project.".to_string()
            ));
        }

        // Determine environment/workspace
        let env_id = options.env_id.as_ref().ok_or_else(|| {
            EnvieError::ValidationError("--env is required for destroy".to_string())
        })?;
        let project_name = self.get_project_name(&project_root)?;
        let workspace = format!("{}-{}", project_name, env_id);

        // Validate workspace
        if workspace == "default" {
            return Err(EnvieError::ValidationError(
                "Cannot destroy the 'default' workspace.".to_string()
            ));
        }

        // Determine which units to destroy
        let units_to_destroy_unordered = if let Some(ref unit_name) = options.unit_name {
            // Destroy specific unit(s) with disambiguation (supports path-based groups)
            let matches = discovery.registry.resolve_unit(unit_name);
            resolve_units_with_prompt(matches, unit_name, false)?
        } else {
            // Destroy all units
            discovery.get_all_units()
        };

        // Sort in reverse topological order (dependents before dependencies)
        let all_units_ordered = discovery.get_units_in_dependency_order()?;
        let requested_qualified_names: std::collections::HashSet<_> =
            units_to_destroy_unordered.iter().map(|u| &u.qualified_name).collect();

        let units_to_destroy: Vec<_> = all_units_ordered
            .into_iter()
            .filter(|unit| requested_qualified_names.contains(&unit.qualified_name))
            .rev()
            .collect();

        if options.dry_run {
            if units_to_destroy.len() == 1 {
                self.print_destroy_plan(units_to_destroy[0], &workspace)?;
            } else {
                self.print_destroy_all_plan(&units_to_destroy, &workspace)?;
            }
            return Ok(());
        }

        self.output_manager.section_header(&format!("Destroying {} unit(s) for environment: {}", units_to_destroy.len(), env_id));

        // Load environment config to get backend values
        let environment_config = self.load_environment_config(&project_root)?;
        let resolved_env = crate::common::environment::EnvironmentResolver::new(
            workspace.clone(),
            project_name.clone(),
            environment_config.clone(),
        ).resolve_environment("ephemeral")?;

        // Validate required backend config values exist
        let region = resolved_env.backend.config.get("region")
            .ok_or_else(|| EnvieError::ValidationError("Missing required backend config: region".to_string()))?;
        let bucket = resolved_env.backend.config.get("bucket")
            .ok_or_else(|| EnvieError::ValidationError("Missing required backend config: bucket".to_string()))?;
        let dynamodb_table = resolved_env.backend.config.get("dynamodb_table")
            .ok_or_else(|| EnvieError::ValidationError("Missing required backend config: dynamodb_table".to_string()))?;

        // Prepare terraform variables
        let mut terraform_vars: Vec<(String, String)> = Vec::new();
        terraform_vars.push(("envie_workspace".to_string(), workspace.clone()));
        terraform_vars.push(("envie_backend_region".to_string(), region.clone()));
        terraform_vars.push(("envie_backend_bucket".to_string(), bucket.clone()));
        terraform_vars.push(("envie_backend_dynamodb_table".to_string(), dynamodb_table.clone()));

        let units_count = units_to_destroy.len();
        for unit in units_to_destroy {
            // Check if workspace exists for this unit
            let unit_path = project_root.join(&unit.path);
            let terraform_manager = TerraformManager::new(&unit_path)
                .with_verbose(options.verbose);

            if terraform_manager.workspace_list()?.contains(&workspace) {
                self.destroy_unit(unit, &project_root, &workspace, &terraform_vars, options.verbose).await?;
            } else if options.verbose {
                self.output_manager.warning(&format!("Skipping unit '{}' - workspace '{}' does not exist", unit.config.name, workspace));
            }
        }

        self.output_manager.section_header("Destroy complete");
        self.output_manager.success(&format!("Destroyed {} unit(s) for workspace: {}", units_count, workspace));

        Ok(())
    }
    
    fn print_destroy_plan(&self, unit: &DiscoveredUnit, workspace: &str) -> Result<()> {
        self.output_manager.print_green("🗑️  Destroy Plan (Dry Run)\n");

        println!("Unit to Destroy:");
        println!("  Name: {}", unit.config.name);
        println!("  Type: {:?}", unit.config.unit_type);
        println!("  Path: {}", unit.path.display());
        println!("  Workspace: {}", workspace);
        println!();

        if !unit.config.dependencies.is_empty() {
            println!("⚠️  Note: This unit has dependencies:");
            for dep in &unit.config.dependencies {
                let dep_display = dep.name().map(|n| n.clone()).or_else(|| dep.path().map(|p| p.clone())).unwrap_or_else(|| "unknown".to_string());
                println!("    - {}", dep_display);
            }
            println!("    Dependencies will NOT be destroyed automatically.");
            println!();
        }

        println!("Actions:");
        println!("  1. Select workspace '{}'", workspace);
        println!("  2. Run terraform destroy");
        println!("  3. Delete workspace '{}'", workspace);

        Ok(())
    }

    fn print_destroy_all_plan(&self, units: &[&DiscoveredUnit], workspace: &str) -> Result<()> {
        self.output_manager.print_green("🗑️  Destroy All Units Plan (Dry Run)\n");

        println!("Destruction Order (Reverse Topological):");
        for (i, unit) in units.iter().enumerate() {
            println!("  {}. {} ({:?})", i + 1, unit.config.name, unit.config.unit_type);
            println!("     Path: {}", unit.path.display());
            if !unit.config.dependencies.is_empty() {
                println!("     Dependencies:");
                for dep in &unit.config.dependencies {
                    let dep_display = dep.name().map(|n| n.clone()).or_else(|| dep.path().map(|p| p.clone())).unwrap_or_else(|| "unknown".to_string());
                    println!("       - {}", dep_display);
                }
            }
            println!();
        }

        println!("📊 Summary:");
        println!("  Total units to destroy: {}", units.len());
        println!("  Workspace: {}", workspace);
        println!();
        println!("Actions for each unit:");
        println!("  1. Select workspace '{}'", workspace);
        println!("  2. Run terraform destroy");
        println!("  3. Delete workspace '{}'", workspace);

        Ok(())
    }
    
    async fn destroy_unit(
        &self,
        unit: &DiscoveredUnit,
        project_root: &PathBuf,
        workspace: &str,
        terraform_vars: &[(String, String)],
        verbose: bool,
    ) -> Result<()> {
        self.output_manager.unit_prefix(&unit.config.name, "Destroying");
        if verbose {
            self.output_manager.unit_prefix(&unit.config.name, &format!("Path: {}", unit.path.display()));
            self.output_manager.unit_prefix(&unit.config.name, &format!("Workspace: {}", workspace));
        }

        let unit_path = project_root.join(&unit.path);
        let terraform_manager = TerraformManager::new(&unit_path)
            .with_verbose(verbose);
        
        // Select the workspace
        if verbose {
            self.output_manager.unit_prefix(&unit.config.name, "Selecting workspace");
        }
        terraform_manager.workspace_select(workspace)?;
        
        // Destroy terraform resources with required variables
        if verbose {
            self.output_manager.unit_prefix(&unit.config.name, "Running terraform destroy");
        }
        let vars_refs: Vec<(&str, &str)> = terraform_vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        terraform_manager.destroy(&vars_refs)?;
        
        // Switch back to default workspace
        if verbose {
            self.output_manager.unit_prefix(&unit.config.name, "Switching to default workspace");
        }
        terraform_manager.workspace_select("default")?;
        
        // Delete the workspace
        if verbose {
            self.output_manager.unit_prefix(&unit.config.name, "Deleting workspace");
        }
        terraform_manager.workspace_delete(workspace)?;
        
        self.output_manager.unit_prefix(&unit.config.name, "Destroyed successfully");
        self.output_manager.print_msg("");
        
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
    
    fn get_project_name(&self, project_root: &PathBuf) -> Result<String> {
        let workspace_file = project_root.join("workspace.envie.yaml");
        if !workspace_file.exists() {
            return Ok("envie-project".to_string());
        }

        let content = std::fs::read_to_string(&workspace_file)?;
        let config: crate::common::service_config::WorkspaceConfig = serde_yaml::from_str(&content)?;
        
        Ok(config.project.as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "envie-project".to_string()))
    }
    
    fn load_environment_config(&self, project_root: &PathBuf) -> Result<EnvironmentConfig> {
        let workspace_file = project_root.join("workspace.envie.yaml");
        if !workspace_file.exists() {
            // Return default config
            return Ok(EnvironmentConfig {
                project: None,
                ephemeral: crate::common::environment::EphemeralConfig {
                    naming_pattern: "{project}-{id}".to_string(),
                    backend: crate::common::environment::BackendConfig {
                        backend_type: "local".to_string(),
                        config: HashMap::new(),
                    },
                },
                stable: HashMap::new(),
            });
        }

        let content = std::fs::read_to_string(&workspace_file)?;
        let config: crate::common::service_config::WorkspaceConfig = serde_yaml::from_str(&content)?;
        
        // Use environments from workspace config if available
        if let Some(env_config) = config.environments {
            Ok(env_config)
        } else {
            // Fallback to default
            Ok(EnvironmentConfig {
                project: config.project.clone(),
                ephemeral: crate::common::environment::EphemeralConfig {
                    naming_pattern: "{project}-{id}".to_string(),
                    backend: crate::common::environment::BackendConfig {
                        backend_type: "local".to_string(),
                        config: HashMap::new(),
                    },
                },
                stable: HashMap::new(),
            })
        }
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
}
