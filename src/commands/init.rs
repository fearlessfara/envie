use crate::common::Result;
use crate::common::service_config::{ProjectInfo, WorkspaceConfig, ServiceConfig};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct InitOptions {
    pub project: bool,
    pub unit: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub verbose: bool,
}

pub struct InitCommand {
    working_directory: std::path::PathBuf,
}

impl InitCommand {
    pub fn new(working_directory: std::path::PathBuf) -> Self {
        Self { working_directory }
    }

    pub async fn execute(&self, options: InitOptions) -> Result<()> {
        if options.project {
            self.init_project(&options).await
        } else if let Some(unit_path) = &options.unit {
            self.init_unit(unit_path, &options).await
        } else {
            Err(crate::common::EnvieError::ValidationError(
                "Must specify either --project or --unit".to_string()
            ))
        }
    }

    async fn init_project(&self, options: &InitOptions) -> Result<()> {
        if options.verbose {
            println!("🚀 Initializing Envie workspace...");
        }

        // Check if workspace.envie.yaml already exists
        let workspace_file = self.working_directory.join("workspace.envie.yaml");
        if workspace_file.exists() {
            return Err(crate::common::EnvieError::ProcessError(
                "Workspace already initialized (workspace.envie.yaml exists)".to_string()
            ));
        }

        // Get project name
        let name = options.name.clone().unwrap_or_else(|| "myproject".to_string());
        let description = options.description.clone().unwrap_or_else(|| "My Envie project".to_string());

        let project_info = ProjectInfo { name, description };

        // Create minimal workspace config
        let workspace_config = WorkspaceConfig {
            version: "1.0".to_string(),
            project: Some(project_info),
            services: vec![],
            defaults: HashMap::new(),
            environments: None,
        };

        // Write workspace.envie.yaml
        let content = serde_yaml::to_string(&workspace_config)?;
        std::fs::write(workspace_file, content)?;

        println!("✅ Workspace initialized!");
        println!("📝 Created workspace.envie.yaml");
        println!("\n🚀 Next steps:");
        println!("  1. Create units: mkdir -p services/myunit");
        println!("  2. Initialize unit: envie init --unit services/myunit");
        println!("  3. Add Terraform code to services/myunit/main.tf");

        Ok(())
    }

    async fn init_unit(&self, unit_path: &str, options: &InitOptions) -> Result<()> {
        if options.verbose {
            println!("🚀 Initializing unit at {}...", unit_path);
        }

        let unit_dir = self.working_directory.join(unit_path);

        // Create directory if it doesn't exist
        if !unit_dir.exists() {
            std::fs::create_dir_all(&unit_dir)?;
            if options.verbose {
                println!("📁 Created directory: {}", unit_path);
            }
        }

        // Check if envie.yaml already exists
        let envie_yaml = unit_dir.join("envie.yaml");
        if envie_yaml.exists() {
            return Err(crate::common::EnvieError::ProcessError(
                format!("Unit already initialized (envie.yaml exists in {})", unit_path)
            ));
        }

        // Get unit name from path (last component)
        let unit_name = unit_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unit")
            .to_string();

        let name = options.name.clone().unwrap_or(unit_name.clone());

        // Create basic unit config
        let unit_config = ServiceConfig {
            name: name.clone(),
            description: format!("{} unit", name),
            modules: vec![],
            dependencies: vec![],
        };

        // Write envie.yaml
        let content = serde_yaml::to_string(&unit_config)?;
        std::fs::write(envie_yaml, content)?;

        println!("✅ Unit initialized!");
        println!("📝 Created {}/envie.yaml", unit_path);
        println!("\n🚀 Next steps:");
        println!("  1. Add Terraform code to {}/main.tf", unit_path);
        println!("  2. Configure dependencies in {}/envie.yaml", unit_path);
        println!("  3. Deploy: envie deploy --unit {} --env <env-id>", name);

        Ok(())
    }
}
