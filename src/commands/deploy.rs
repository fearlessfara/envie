use crate::common::*;
use crate::common::environment::{EnvironmentConfig, EphemeralConfig, BackendConfig as EnvironmentBackendConfig};
use crate::common::service_config::WorkspaceConfig;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DeployOptions {
    pub unit_name: Option<String>,
    pub env_id: String,
    pub environment_overrides: HashMap<String, String>,
    pub dry_run: bool,
    pub no_prompt: bool,
    pub verbose: bool,
}

pub struct DeployCommand {
    working_directory: PathBuf,
    output_manager: OutputManager,
}

impl DeployCommand {
    pub fn new(working_directory: PathBuf) -> Self {
        Self {
            working_directory,
            output_manager: OutputManager::new(),
        }
    }
    
    pub async fn execute(&self, options: DeployOptions) -> Result<()> {
        // Find the project root (where workspace.envie is)
        let project_root = self.find_project_root()?;
        
        if options.verbose {
            println!("🚀 Starting deployment with flexible unit discovery...");
            println!("📂 Project root: {}", project_root.display());
            println!("📂 Current directory: {}", std::env::current_dir()?.display());
        }

        // Discover all units from the project root
        let mut discovery = UnitDiscovery::new(project_root.clone());
        discovery.discover_all()?;
        
        if discovery.registry.units.is_empty() {
            return Err(EnvieError::ValidationError(
                "No deployable units found. Make sure you have envie.yaml files in your project.".to_string()
            ));
        }
        
        // Determine which unit(s) to deploy
        let units_to_deploy = if let Some(ref unit_name) = options.unit_name {
            // Deploy specific unit(s) with disambiguation
            // Supports single units, ambiguous names, or path-based groups
            let matches = discovery.registry.resolve_unit(unit_name);
            resolve_units_with_prompt(matches, unit_name, options.no_prompt)?
        } else {
            // Check if we're in a unit directory first
            let current_dir = std::env::current_dir()?;
            let mut search_path = current_dir.clone();
            let mut found_unit = None;
            
            // Try to find a unit at or above the current directory
            loop {
                if let Ok(relative_path) = search_path.strip_prefix(&project_root) {
                    let relative_pathbuf = relative_path.to_path_buf();
                    if options.verbose {
                        println!("🔍 Searching for unit at path: {:?}", relative_pathbuf);
                    }
                    if let Some(unit) = discovery.registry.get_unit_by_path(&relative_pathbuf) {
                        if options.verbose {
                            println!("✅ Found unit: {} at path: {}", unit.config.name, unit.path.display());
                        }
                        found_unit = Some(unit);
                        break;
                    }
                }
                
                // Move up one directory
                if let Some(parent) = search_path.parent() {
                    search_path = parent.to_path_buf();
                } else {
                    break;
                }
            }
            
            if let Some(unit) = found_unit {
                // Deploy the unit in the current directory
                vec![unit]
            } else {
                // Deploy all units in dependency order
                if options.verbose {
                    println!("🚀 No specific unit found, deploying all units in dependency order...");
                    println!("🔍 All units: {:?}", discovery.registry.get_all_units().iter().map(|u| format!("{} (level: {})", u.config.name, u.level)).collect::<Vec<_>>());
                    println!("🔍 Root units: {:?}", discovery.get_root_units().iter().map(|u| format!("{} (level: {})", u.config.name, u.level)).collect::<Vec<_>>());
                }
                let units = discovery.get_units_in_dependency_order()?;
                if options.verbose {
                    println!("🔍 Units in dependency order: {:?}", units.iter().map(|u| &u.config.name).collect::<Vec<_>>());
                }
                units
            }
        };
        
        // Resolve workspace name
        let project_name = self.get_project_name()?;
        let workspace = format!("{}-{}", project_name, options.env_id);
        
        if options.verbose {
            println!("📦 Deploying {} unit(s)", units_to_deploy.len());
            println!("🌍 Workspace: {}", workspace);
        }
        
        // Load environment configuration
        let environment_config = self.load_environment_config()?;
        
        // Check and setup backend infrastructure if needed
        self.ensure_backend_exists(&environment_config, options.no_prompt, options.verbose).await?;
        
        // Create environment resolver
        let environment_resolver = EnvironmentResolver::new(
            workspace.clone(),
            project_name.clone(),
            environment_config,
        ).with_available_workspaces(self.get_available_workspaces()?);
        
        // Resolve deployment order (dependencies first)
        let deployment_order = if units_to_deploy.len() == 1 {
            // Single unit - resolve its dependencies
            discovery.resolve_deployment_order(&units_to_deploy[0].config.name)?
        } else {
            // Multiple units - sort them topologically
            // Get all units in dependency order, then filter to only those requested
            let all_units_ordered = discovery.get_units_in_dependency_order()?;
            let requested_qualified_names: std::collections::HashSet<_> =
                units_to_deploy.iter().map(|u| &u.qualified_name).collect();

            all_units_ordered
                .into_iter()
                .filter(|unit| requested_qualified_names.contains(&unit.qualified_name))
                .collect()
        };
        
        if options.dry_run {
            self.print_deployment_plan(&deployment_order, &environment_resolver)?;
            return Ok(());
        }
        
        // Deploy each unit in order
        self.output_manager.print_green(&format!("\n🚀 Deploying {} unit(s)...\n", deployment_order.len()));
        
        for (index, unit) in deployment_order.iter().enumerate() {
            self.output_manager.print_blue(&format!("[{}/{}] Deploying: {}", 
                index + 1, 
                deployment_order.len(), 
                unit.config.name
            ));
            
            self.deploy_unit(
                unit,
                &project_root,
                &workspace,
                &environment_resolver,
                &options.environment_overrides,
                &options,
            ).await?;
        }
        
        self.output_manager.print_green("\n✅ Deployment complete!");
        
        Ok(())
    }
    
    async fn deploy_unit(
        &self,
        unit: &DiscoveredUnit,
        project_root: &PathBuf,
        workspace: &str,
        environment_resolver: &EnvironmentResolver,
        environment_overrides: &HashMap<String, String>,
        options: &DeployOptions,
    ) -> Result<()> {
        println!("  📍 Path: {}", unit.path.display());
        println!("  🏷️  Type: {:?}", unit.config.unit_type);
        println!("  💾 State: {:?}", unit.config.state_management);
        
        // Get the full path to the unit
        let unit_path = project_root.join(&unit.path);
        
        // Convert dependencies to the format expected by TerraformGenerator
        let dependencies: Vec<crate::common::service_config::DependencyReference> = unit.config.depends.iter().map(|dep| {
            crate::common::service_config::DependencyReference {
                path: dep.path.clone(),
                environment: dep.environment.clone(),
            }
        }).collect();
        
        // Generate Terraform files
        let generator = TerraformGenerator::new();
        let project_name = self.get_project_name()?;
        generator.write_generated_files(
            &unit_path,
            &dependencies,
            &self.convert_unit_to_module_config(unit),
            environment_resolver,
            environment_overrides,
            &unit.config.name,
            &unit.config.name,
            &project_name,
            &options.env_id,
        )?;
        
        // Initialize and apply Terraform
        let terraform_manager = TerraformManager::new(&unit_path);
        
        println!("  🔧 Running terraform init...");
        terraform_manager.init()?;
        
        // Create or select workspace
        let workspaces = terraform_manager.workspace_list()?;
        if workspaces.iter().any(|w| w == workspace) {
            terraform_manager.workspace_select(workspace)?;
        } else {
            terraform_manager.workspace_new(workspace)?;
        }
        
        // Apply Terraform
        println!("  ⚡ Running terraform apply...");
        terraform_manager.apply(&[])?;
        
        println!("  ✅ Unit deployed successfully\n");
        
        Ok(())
    }
    
    fn print_deployment_plan(
        &self,
        deployment_order: &[&DiscoveredUnit],
        _environment_resolver: &EnvironmentResolver,
    ) -> Result<()> {
        self.output_manager.print_green("📋 Deployment Plan (Dry Run)\n");
        
        println!("Deployment Order:");
        for (index, unit) in deployment_order.iter().enumerate() {
            println!("  {}. {} ({:?})", 
                index + 1, 
                unit.config.name, 
                unit.config.unit_type
            );
            println!("     Path: {}", unit.path.display());
            println!("     State: {:?}", unit.config.state_management);
            
            if !unit.config.depends.is_empty() {
                println!("     Dependencies:");
                for dep in &unit.config.depends {
                    println!("       - {}", dep.path);
                }
            }
            println!();
        }
        
        println!("📊 Summary:");
        println!("  Total units to deploy: {}", deployment_order.len());
        
        // Count by type
        let mut type_counts: HashMap<String, usize> = HashMap::new();
        for unit in deployment_order {
            let type_name = format!("{:?}", unit.config.unit_type);
            *type_counts.entry(type_name).or_insert(0) += 1;
        }
        
        for (unit_type, count) in type_counts {
            println!("  {}: {}", unit_type, count);
        }
        
        Ok(())
    }
    
    fn convert_unit_to_module_config(&self, unit: &DiscoveredUnit) -> ModuleConfig {
        // Convert UnitConfig to ModuleConfig for backward compatibility with TerraformGenerator
        ModuleConfig {
            name: unit.config.name.clone(),
            description: unit.config.description.clone(),
            path: unit.path.to_string_lossy().to_string(),
            depends: unit.config.depends.iter().map(|dep| {
                crate::common::service_config::DependencyReference {
                    path: dep.path.clone(),
                    environment: dep.environment.clone(),
                }
            }).collect(),
            state_management: self.convert_state_management(&unit.config.state_management),
        }
    }
    
    fn convert_state_management(&self, sm: &crate::common::unit_config::StateManagement) -> crate::common::service_config::StateManagement {
        use crate::common::unit_config::StateManagement as UnitSM;
        use crate::common::service_config::StateManagement as ServiceSM;
        
        match sm {
            UnitSM::Dedicated => ServiceSM::Dedicated,
            UnitSM::Parent => ServiceSM::Service, // Map parent to service for now
            UnitSM::Shared(id) => ServiceSM::Shared(id.clone()),
            UnitSM::Group(id) => ServiceSM::Shared(id.clone()), // Map group to shared
        }
    }
    
    fn get_project_name(&self) -> Result<String> {
        let workspace_file = self.working_directory.join("workspace.envie");
        if !workspace_file.exists() {
            return Ok("envie-project".to_string());
        }

        let content = std::fs::read_to_string(&workspace_file)?;
        let config: WorkspaceConfig = serde_yaml::from_str(&content)?;
        
        Ok(config.project.as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "envie-project".to_string()))
    }
    
    fn load_environment_config(&self) -> Result<EnvironmentConfig> {
        let workspace_file = self.working_directory.join("workspace.envie");
        if !workspace_file.exists() {
            // Return default config
            return Ok(EnvironmentConfig {
                project: None,
                ephemeral: EphemeralConfig {
                    naming_pattern: "{project}-{id}".to_string(),
                    backend: EnvironmentBackendConfig {
                        backend_type: "local".to_string(),
                        config: HashMap::new(),
                    },
                },
                stable: HashMap::new(),
            });
        }

        let content = std::fs::read_to_string(&workspace_file)?;
        let config: WorkspaceConfig = serde_yaml::from_str(&content)?;
        
        // Use environments from workspace config if available
        if let Some(env_config) = config.environments {
            Ok(env_config)
        } else {
            // Fallback to default
            Ok(EnvironmentConfig {
                project: config.project.clone(),
                ephemeral: EphemeralConfig {
                    naming_pattern: "{project}-{id}".to_string(),
                    backend: EnvironmentBackendConfig {
                        backend_type: "local".to_string(),
                        config: HashMap::new(),
                    },
                },
                stable: HashMap::new(),
            })
        }
    }
    
    fn get_available_workspaces(&self) -> Result<Vec<String>> {
        // For now, return empty list
        // In a real implementation, this would query Terraform or the state backend
        Ok(Vec::new())
    }
    
    /// Find the project root by searching up for workspace.envie
    fn find_project_root(&self) -> Result<PathBuf> {
        let mut current = self.working_directory.clone();
        
        loop {
            let workspace_file = current.join("workspace.envie");
            if workspace_file.exists() {
                return Ok(current);
            }
            
            // Move up one directory
            if let Some(parent) = current.parent() {
                current = parent.to_path_buf();
            } else {
                // No workspace.envie found, use working directory as fallback
                return Ok(self.working_directory.clone());
            }
        }
    }
    
    /// Ensure backend infrastructure (S3 bucket + DynamoDB table) exists
    async fn ensure_backend_exists(
        &self,
        environment_config: &EnvironmentConfig,
        no_prompt: bool,
        verbose: bool,
    ) -> Result<()> {
        // Get backend config from ephemeral environment
        let backend = &environment_config.ephemeral.backend;
        
        // Only handle S3 backend for now
        if backend.backend_type != "s3" {
            if verbose {
                println!("⚠️  Skipping backend check for non-S3 backend type: {}", backend.backend_type);
            }
            return Ok(());
        }
        
        // Extract required values
        let bucket_name = backend.config.get("bucket")
            .ok_or_else(|| EnvieError::ValidationError("S3 bucket name not found in backend config".to_string()))?
            .to_string();
        
        let dynamodb_table = backend.config.get("dynamodb_table")
            .ok_or_else(|| EnvieError::ValidationError("DynamoDB table name not found in backend config".to_string()))?
            .to_string();
        
        let region = backend.config.get("region")
            .ok_or_else(|| EnvieError::ValidationError("AWS region not found in backend config".to_string()))?
            .to_string();
        
        // Check if backend infrastructure exists
        let bootstrap = BackendBootstrap::new(bucket_name, dynamodb_table, region);
        let status = bootstrap.check_exists()?;
        
        if status.is_ready() {
            if verbose {
                println!("✅ Backend infrastructure already exists");
            }
            return Ok(());
        }
        
        // Backend infrastructure doesn't exist - create it
        bootstrap.create(no_prompt)?;
        
        Ok(())
    }
}

