use crate::common::*;
use std::path::PathBuf;
use std::io::{self, Write};

#[derive(Debug, Clone)]
pub struct DeleteOptions {
    pub unit_name: Option<String>,
    pub env_id: String,
    pub dry_run: bool,
    pub no_prompt: bool,
    pub verbose: bool,
}

pub struct DeleteCommand {
    working_directory: PathBuf,
    output_manager: OutputManager,
}

impl DeleteCommand {
    pub fn new(working_directory: PathBuf) -> Self {
        Self {
            working_directory,
            output_manager: OutputManager::new(),
        }
    }

    pub async fn execute(&self, options: DeleteOptions) -> Result<()> {
        // Find the project root
        let project_root = self.find_project_root()?;

        if options.verbose {
            println!("🗑️  Starting complete deletion for environment: {}", options.env_id);
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

        let project_name = self.get_project_name(&project_root)?;
        let workspace = format!("{}-{}", project_name, options.env_id);

        // Validate workspace
        if workspace == "default" {
            return Err(EnvieError::ValidationError(
                "Cannot delete the 'default' workspace.".to_string()
            ));
        }

        // Get units to delete
        let units_in_order = discovery.get_units_in_dependency_order()?;
        let units_to_delete: Vec<_> = units_in_order.into_iter().rev().collect();

        if options.dry_run {
            self.print_delete_plan(&units_to_delete, &workspace, &options.env_id)?;
            return Ok(());
        }

        // Prompt for confirmation unless --no-prompt
        if !options.no_prompt {
            self.output_manager.print_yellow(&format!(
                "\n⚠️  WARNING: This will permanently delete all resources and state management infrastructure for environment: {}\n",
                options.env_id
            ));
            println!("This includes:");
            println!("  • All Terraform resources across {} units", units_to_delete.len());
            println!("  • S3 bucket: {}-state-{}", project_name, options.env_id);
            println!("  • DynamoDB table: {}-locks-{}", project_name, options.env_id);
            println!("\n⚠️  This action CANNOT be undone!");

            print!("\nType 'yes' to confirm deletion: ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            if input.trim() != "yes" {
                println!("❌ Deletion cancelled.");
                return Ok(());
            }
        }

        self.output_manager.print_green(&format!("🗑️  Deleting environment: {}\n", options.env_id));

        // Step 1: Destroy all terraform resources
        self.output_manager.print_blue("Step 1: Destroying Terraform resources...\n");

        for unit in &units_to_delete {
            let unit_path = project_root.join(&unit.path);
            let terraform_manager = TerraformManager::new(&unit_path);

            if terraform_manager.workspace_list()?.contains(&workspace) {
                self.destroy_unit(unit, &project_root, &workspace).await?;
            } else if options.verbose {
                println!("⏭️  Skipping unit '{}' - workspace '{}' does not exist\n", unit.config.name, workspace);
            }
        }

        // Step 2: Delete backend infrastructure
        self.output_manager.print_blue("Step 2: Deleting state management infrastructure...\n");
        self.delete_backend_infrastructure(&project_name, &options.env_id).await?;

        self.output_manager.print_green(&format!("\n✅ Successfully deleted environment: {}", options.env_id));
        println!("   All resources and state management infrastructure have been removed.");

        Ok(())
    }

    fn print_delete_plan(&self, units: &[&DiscoveredUnit], workspace: &str, env_id: &str) -> Result<()> {
        self.output_manager.print_green("🗑️  Complete Deletion Plan (Dry Run)\n");

        println!("Environment: {}", env_id);
        println!("Workspace: {}", workspace);
        println!();

        println!("Step 1: Destroy Terraform Resources");
        println!("Destruction Order (Reverse Topological):");
        for (i, unit) in units.iter().enumerate() {
            println!("  {}. {} ({:?})", i + 1, unit.config.name, unit.config.unit_type);
            println!("     Path: {}", unit.path.display());
            if !unit.config.depends.is_empty() {
                println!("     Dependencies:");
                for dep in &unit.config.depends {
                    println!("       - {}", dep.path);
                }
            }
            println!();
        }

        println!("Step 2: Delete State Management Infrastructure");
        println!("  • S3 Bucket: Will be emptied and deleted");
        println!("  • DynamoDB Table: Will be deleted");
        println!();

        println!("📊 Summary:");
        println!("  Total units to destroy: {}", units.len());
        println!("  Backend infrastructure: S3 + DynamoDB");

        Ok(())
    }

    async fn destroy_unit(
        &self,
        unit: &DiscoveredUnit,
        project_root: &PathBuf,
        workspace: &str,
    ) -> Result<()> {
        println!("🗑️  Destroying unit: {}", unit.config.name);
        println!("  📍 Path: {}", unit.path.display());
        println!("  🌍 Workspace: {}", workspace);

        let unit_path = project_root.join(&unit.path);
        let terraform_manager = TerraformManager::new(&unit_path);

        // Select the workspace
        println!("  🔧 Selecting workspace...");
        terraform_manager.workspace_select(workspace)?;

        // Destroy terraform resources
        println!("  💥 Running terraform destroy...");
        terraform_manager.destroy(&[])?;

        // Switch back to default workspace
        println!("  🔧 Switching to default workspace...");
        terraform_manager.workspace_select("default")?;

        // Delete the workspace
        println!("  🗑️  Deleting workspace...");
        terraform_manager.workspace_delete(workspace)?;

        println!("  ✅ Unit destroyed successfully\n");

        Ok(())
    }

    async fn delete_backend_infrastructure(&self, project_name: &str, env_id: &str) -> Result<()> {
        let bucket_name = format!("{}-state-{}", project_name, env_id);
        let table_name = format!("{}-locks-{}", project_name, env_id);

        // Delete S3 bucket (must be emptied first)
        println!("🗑️  Deleting S3 bucket: {}", bucket_name);

        // First, empty the bucket
        let empty_output = std::process::Command::new("aws")
            .args(&["s3", "rm", &format!("s3://{}", bucket_name), "--recursive"])
            .output()?;

        if !empty_output.status.success() {
            let stderr = String::from_utf8_lossy(&empty_output.stderr);
            // Don't fail if bucket doesn't exist
            if !stderr.contains("NoSuchBucket") {
                return Err(EnvieError::ProcessError(
                    format!("Failed to empty S3 bucket: {}", stderr)
                ));
            } else {
                println!("  ⏭️  Bucket does not exist, skipping...");
            }
        } else {
            // Then delete the bucket
            let delete_output = std::process::Command::new("aws")
                .args(&["s3api", "delete-bucket", "--bucket", &bucket_name])
                .output()?;

            if !delete_output.status.success() {
                let stderr = String::from_utf8_lossy(&delete_output.stderr);
                if !stderr.contains("NoSuchBucket") {
                    return Err(EnvieError::ProcessError(
                        format!("Failed to delete S3 bucket: {}", stderr)
                    ));
                }
            } else {
                println!("  ✅ S3 bucket deleted");
            }
        }

        // Delete DynamoDB table
        println!("🗑️  Deleting DynamoDB table: {}", table_name);

        let table_output = std::process::Command::new("aws")
            .args(&["dynamodb", "delete-table", "--table-name", &table_name])
            .output()?;

        if !table_output.status.success() {
            let stderr = String::from_utf8_lossy(&table_output.stderr);
            // Don't fail if table doesn't exist
            if !stderr.contains("ResourceNotFoundException") {
                return Err(EnvieError::ProcessError(
                    format!("Failed to delete DynamoDB table: {}", stderr)
                ));
            } else {
                println!("  ⏭️  Table does not exist, skipping...");
            }
        } else {
            println!("  ✅ DynamoDB table deleted");
        }

        Ok(())
    }

    fn find_project_root(&self) -> Result<PathBuf> {
        let mut current = self.working_directory.clone();

        loop {
            let workspace_file = current.join("workspace.envie");
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
        let workspace_file = project_root.join("workspace.envie");
        if !workspace_file.exists() {
            return Ok("envie-project".to_string());
        }

        let content = std::fs::read_to_string(&workspace_file)?;
        let config: crate::common::service_config::WorkspaceConfig = serde_yaml::from_str(&content)?;

        Ok(config.project.as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "envie-project".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_delete_command_creation() {
        let temp_dir = TempDir::new().unwrap();
        let deleter = DeleteCommand::new(temp_dir.path().to_path_buf());
        assert_eq!(deleter.working_directory, temp_dir.path());
    }
}
