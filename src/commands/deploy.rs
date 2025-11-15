use crate::common::*;
use crate::common::environment::{EnvironmentConfig, EphemeralConfig, BackendConfig as EnvironmentBackendConfig};
use crate::common::service_config::WorkspaceConfig;
use std::collections::HashMap;
use std::path::PathBuf;
use regex::Regex;

#[derive(Debug, Clone)]
pub struct DeployOptions {
    pub unit_name: Option<String>,
    pub env_id: String,
    pub environment_overrides: HashMap<String, String>,
    pub dry_run: bool,
    pub no_prompt: bool,
    pub verbose: bool,
    pub reconfigure: bool,
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
        // Find the project root (where workspace.envie.yaml is)
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

        // Load environment configuration for validation
        let environment_config = self.load_environment_config()?;
        
        if options.verbose {
            println!("🔍 Environment config loaded:");
            println!("  Ephemeral backend type: {}", environment_config.ephemeral.backend.backend_type);
            println!("  Ephemeral backend config: {:?}", environment_config.ephemeral.backend.config);
        }

        // Get available stable environments for prompting
        let available_stable_envs: Vec<String> = environment_config.stable.keys().cloned().collect();
        
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
        
        // Validate environment ID to prevent path traversal
        self.validate_env_id(&options.env_id)?;
        
        // Resolve workspace name
        let project_name = self.get_project_name()?;
        let workspace = format!("{}-{}", project_name, options.env_id);

        if options.verbose {
            println!("📦 Deploying {} unit(s)", units_to_deploy.len());
            println!("🌍 Workspace: {}", workspace);
        }

        // Create environment resolver
        let environment_resolver = EnvironmentResolver::new(
            workspace.clone(),
            project_name.clone(),
            environment_config.clone(),
        ).with_available_workspaces(self.get_available_workspaces()?);

        // Clone registry early to avoid lifetime issues
        let registry = discovery.registry.clone();

        // Resolve deployment order (dependencies first) - must be done before cloning registry
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

        // Group units by dependency level for parallel deployment - must be done before cloning registry
        let deployment_levels = discovery.group_units_by_level(&deployment_order)?;
        
        // Clone registry after we're done using discovery
        let registry = discovery.registry.clone();

        if options.dry_run {
            self.print_deployment_plan(&deployment_order, &environment_resolver, &options.environment_overrides, &options.env_id)?;
            return Ok(());
        }

        // Check and setup backend infrastructure if needed (only for actual deployment)
        self.ensure_backend_exists(&environment_config, options.no_prompt, options.verbose).await?;

        // Print environment resolution if verbose
        if options.verbose {
            self.print_environment_resolution(&deployment_order, &environment_resolver, &options.environment_overrides, &options.env_id, &registry)?;
        }
        let total_units = deployment_order.len();
        
        self.output_manager.section_header(&format!("Deploying {} unit(s)", total_units));
        
        let mut deployed_count = 0;
        
        for level in deployment_levels.iter() {
            if level.len() == 1 {
                // Single unit in this level - deploy sequentially for cleaner output
                deployed_count += 1;
                self.output_manager.progress(deployed_count, total_units, &level[0].config.name);

                self.deploy_unit(
                    level[0],
                    &project_root,
                    &workspace,
                    &environment_resolver,
                    &options.environment_overrides,
                    &options,
                    &registry,
                ).await?;
            } else {
                // Multiple units in this level - deploy in parallel
                // Show progress for all units first
                for unit in level {
                    deployed_count += 1;
                    self.output_manager.progress(deployed_count, total_units, &unit.config.name);
                }
                
                // Deploy all units in this level in parallel using tokio::spawn
                let mut tasks = Vec::new();
                let mut task_count = 0;
                for unit in level {
                    task_count += 1;
                    // Look up unit from registry to avoid borrowing from discovery
                    let unit_name = unit.config.name.clone();
                    let unit_from_registry = registry.get_unit(&unit_name)
                        .ok_or_else(|| EnvieError::ValidationError(format!("Unit '{}' not found in registry", unit_name)))?;
                    let unit_clone = unit_from_registry.clone();
                    let project_root_clone = project_root.clone();
                    let workspace_clone = workspace.clone();
                    let env_resolver = environment_resolver.clone();
                    let env_overrides = options.environment_overrides.clone();
                    let options_clone = options.clone();
                    let registry_clone = registry.clone();
                    let working_dir = self.working_directory.clone();
                    
                    let task = tokio::spawn(async move {
                        Self::deploy_unit_parallel(
                            &unit_clone,
                            &project_root_clone,
                            &workspace_clone,
                            &env_resolver,
                            &env_overrides,
                            &options_clone,
                            &registry_clone,
                            &working_dir,
                        ).await
                    });
                    tasks.push(task);
                }
                
                // Wait for all parallel deployments to complete and collect errors
                let mut errors = Vec::new();
                let mut completed = 0;
                for task in tasks {
                    match task.await {
                        Ok(Ok(())) => {
                            completed += 1;
                        },
                        Ok(Err(e)) => {
                            errors.push(e);
                        },
                        Err(e) => {
                            errors.push(EnvieError::ProcessError(format!("Task join error: {}", e)));
                        },
                    }
                }
                
                // Report all errors if any occurred
                if !errors.is_empty() {
                    if errors.len() == 1 {
                        return Err(errors.into_iter().next().unwrap());
                    } else {
                        let error_msg = errors.iter()
                            .map(|e| format!("  - {}", e))
                            .collect::<Vec<_>>()
                            .join("\n");
                        return Err(EnvieError::ValidationError(
                            format!("Multiple units failed during parallel deployment:\n{}", error_msg)
                        ));
                    }
                }
                
                // Progress already shown above, just mark completion
                deployed_count += level.len();
            }
        }
        
        self.output_manager.section_header("Deployment complete");
        self.output_manager.success(&format!("All {} unit(s) deployed successfully", total_units));
        
        Ok(())
    }
    
    /// Resolve the environment to use for a dependency
    /// Priority: CLI override > same as current deployment
    fn resolve_dependency_environment(
        &self,
        dep_unit_name: &str,
        environment_overrides: &HashMap<String, String>,
        default_env: &str,
    ) -> Result<String> {
        // Check if there's a CLI override using the unit name
        if let Some(override_env) = environment_overrides.get(dep_unit_name) {
            Ok(override_env.clone())
        } else {
            // Default to same environment as current deployment
            Ok(default_env.to_string())
        }
    }

    /// Deploy a unit (can be called in parallel)
    async fn deploy_unit_parallel(
        unit: &DiscoveredUnit,
        project_root: &PathBuf,
        workspace: &str,
        environment_resolver: &EnvironmentResolver,
        environment_overrides: &HashMap<String, String>,
        options: &DeployOptions,
        unit_registry: &UnitRegistry,
        working_directory: &PathBuf,
    ) -> Result<()> {
        let unit_name = &unit.config.name;
        let output_manager = OutputManager::new();
        if options.verbose {
            output_manager.unit_prefix(unit_name, &format!("Path: {}", unit.path.display()));
            output_manager.unit_prefix(unit_name, &format!("Type: {:?}", unit.config.unit_type));
            output_manager.unit_prefix(unit_name, &format!("State: {:?}", unit.config.state_management));
        }

        // Get the full path to the unit
        let unit_path = project_root.join(&unit.path);

        // Create a temporary DeployCommand for helper methods
        let deploy_cmd = DeployCommand::new(working_directory.clone());

        // Convert dependencies to the format expected by TerraformGenerator
        let dependencies: Vec<crate::common::service_config::DependencyReference> = unit.config.dependencies.iter().map(|dep| -> Result<_> {
            // Resolve dependency to get path and unit name
            let (dep_path, dep_unit_name) = if let Some(name) = dep.name() {
                // Name-based dependency - look up the unit to get its path
                if let Some(dep_unit) = unit_registry.get_unit(name) {
                    (dep_unit.path.to_string_lossy().to_string(), name.clone())
                } else {
                    return Err(EnvieError::ValidationError(
                        format!("Dependency unit '{}' not found", name)
                    ));
                }
            } else if let Some(path) = dep.path() {
                // Path-based dependency - extract unit name from path
                let (service, _) = deploy_cmd.extract_service_module_from_path(path)?;
                (path.clone(), service)
            } else {
                return Err(EnvieError::ValidationError(
                    "Dependency must have either 'name' or 'path'".to_string()
                ));
            };

            let environment = deploy_cmd.resolve_dependency_environment(&dep_unit_name, environment_overrides, &options.env_id)
                .unwrap_or_else(|_| options.env_id.clone());

            Ok(crate::common::service_config::DependencyReference {
                path: dep_path,
                environment,
            })
        }).collect::<Result<Vec<_>>>()?;
        
        // Generate Terraform files
        let generator = TerraformGenerator::new();
        let project_name = deploy_cmd.get_project_name()?;
        generator.write_generated_files(
            &unit_path,
            &dependencies,
            &deploy_cmd.convert_unit_to_module_config(unit, environment_overrides, &options.env_id, unit_registry),
            environment_resolver,
            environment_overrides,
            &unit.config.name,
            &unit.config.name,
            &project_name,
            &options.env_id,
        )?;
        
        // Initialize and apply Terraform
        let terraform_manager = TerraformManager::new(&unit_path)
            .with_verbose(options.verbose)
            .with_output_prefix(Some(unit.config.name.clone()));

        // Prepare backend configuration and variables
        let resolved_env = environment_resolver.resolve_environment("ephemeral")?;

        // Build backend configuration for terraform init
        let mut backend_config: Vec<(String, String)> = Vec::new();

        // Generate state key based on state management strategy
        let state_key = match &unit.config.state_management {
            crate::common::unit_config::StateManagement::Dedicated => {
                let module_part = if unit.config.name.is_empty() {
                    &unit.config.name
                } else {
                    &unit.config.name
                };
                format!("ephemeral/{}/{}/{}/terraform.tfstate", workspace, unit.config.name, module_part)
            }
            crate::common::unit_config::StateManagement::Parent => {
                format!("ephemeral/{}/{}/service/terraform.tfstate", workspace, unit.config.name)
            }
            crate::common::unit_config::StateManagement::Shared(shared_id) => {
                format!("ephemeral/{}/{}/shared/terraform.tfstate", workspace, shared_id)
            }
            crate::common::unit_config::StateManagement::Group(group_id) => {
                format!("ephemeral/{}/{}/shared/terraform.tfstate", workspace, group_id)
            }
        };

        backend_config.push(("key".to_string(), state_key));

        // Add other backend config values
        for (key, value) in &resolved_env.backend.config {
            if key == "key_pattern" || key == "key" {
                continue; // Already handled above
            }
            backend_config.push((key.clone(), value.clone()));
        }

        // Run terraform init with backend config
        let backend_config_refs: Vec<(&str, &str)> = backend_config.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        terraform_manager.init_with_backend_config(&backend_config_refs, options.reconfigure)?;

        // Select or create workspace
        let workspaces = terraform_manager.workspace_list()?;
        if workspaces.contains(&workspace.to_string()) {
            terraform_manager.workspace_select(workspace)?;
        } else {
            terraform_manager.workspace_new(workspace)?;
        }

        // Prepare terraform variables
        let mut terraform_vars: Vec<(String, String)> = Vec::new();
        terraform_vars.push(("envie_workspace".to_string(), workspace.to_string()));
        
        // Validate required backend config values exist
        let region = resolved_env.backend.config.get("region")
            .ok_or_else(|| EnvieError::ValidationError("Missing required backend config: region".to_string()))?;
        let bucket = resolved_env.backend.config.get("bucket")
            .ok_or_else(|| EnvieError::ValidationError("Missing required backend config: bucket".to_string()))?;
        let dynamodb_table = resolved_env.backend.config.get("dynamodb_table")
            .ok_or_else(|| EnvieError::ValidationError("Missing required backend config: dynamodb_table".to_string()))?;
        
        terraform_vars.push(("envie_backend_region".to_string(), region.clone()));
        terraform_vars.push(("envie_backend_bucket".to_string(), bucket.clone()));
        terraform_vars.push(("envie_backend_dynamodb_table".to_string(), dynamodb_table.clone()));

        // Run terraform apply
        let terraform_vars_refs: Vec<(&str, &str)> = terraform_vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        terraform_manager.apply(&terraform_vars_refs)?;

        output_manager.unit_prefix(unit_name, "Deployed successfully");

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
        unit_registry: &UnitRegistry,
    ) -> Result<()> {
        Self::deploy_unit_parallel(
            unit,
            project_root,
            workspace,
            environment_resolver,
            environment_overrides,
            options,
            unit_registry,
            &self.working_directory,
        ).await
    }
    
    fn print_environment_resolution(
        &self,
        deployment_order: &[&DiscoveredUnit],
        environment_resolver: &EnvironmentResolver,
        environment_overrides: &HashMap<String, String>,
        env_id: &str,
        unit_registry: &UnitRegistry,
    ) -> Result<()> {
        self.output_manager.print_blue("\n🔍 Resolving dependencies:\n");

        for unit in deployment_order {
            if !unit.config.dependencies.is_empty() {
                println!("  ├─ {}", unit.config.name);

                for (i, dep) in unit.config.dependencies.iter().enumerate() {
                    let is_last = i == unit.config.dependencies.len() - 1;
                    let prefix = if is_last { "  └─" } else { "  ├─" };

                    // Resolve dependency to get unit name and display path
                    let (dep_display, dep_unit_name) = if let Some(name) = dep.name() {
                        (name.clone(), name.clone())
                    } else if let Some(path) = dep.path() {
                        let (service, _) = self.extract_service_module_from_path(path)?;
                        (path.clone(), service)
                    } else {
                        continue;
                    };

                    // Resolve environment for this dependency
                    let environment_to_use = self.resolve_dependency_environment(&dep_unit_name, environment_overrides, env_id)?;

                    let resolved_env = environment_resolver.resolve_environment(&environment_to_use)?;

                    // Show if environment was overridden
                    let override_info = if environment_overrides.contains_key(&dep_unit_name) {
                        format!(" (overridden from default '{}')", env_id)
                    } else {
                        String::new()
                    };

                    println!("{}  {} → {}{}", prefix, dep_display, environment_to_use, override_info);
                    println!("  │     Workspace: {}", resolved_env.workspace);

                    // Generate state key for display - need to get the actual unit to get its path
                    let dep_unit = unit_registry.get_unit(&dep_unit_name);
                    let (source_service, source_module) = if let Some(unit) = dep_unit {
                        // Extract from unit path
                        let path_str = unit.path.to_string_lossy();
                        let (service, module) = self.extract_service_module_from_path(&path_str)?;
                        (service, module)
                    } else {
                        // Fallback to unit name
                        (dep_unit_name.clone(), String::new())
                    };
                    let state_key = environment_resolver.generate_state_key(&resolved_env, &source_service, &source_module);

                    if let Some(bucket) = resolved_env.backend.config.get("bucket") {
                        println!("  │     State: s3://{}/{}", bucket, state_key);
                    }
                }
                println!();
            }
        }

        Ok(())
    }

    fn print_deployment_plan(
        &self,
        deployment_order: &[&DiscoveredUnit],
        environment_resolver: &EnvironmentResolver,
        environment_overrides: &HashMap<String, String>,
        env_id: &str,
    ) -> Result<()> {
        self.output_manager.section_header("Deployment Plan (Dry Run)");

        let project_name = self.get_project_name()?;
        let workspace = format!("{}-{}", project_name, env_id);

        self.output_manager.print_msg(&format!("  Environment: {}", env_id));
        self.output_manager.print_msg(&format!("  Workspace:   {}", workspace));
        self.output_manager.print_msg("");

        // Show dependency resolution
        self.output_manager.print_bold("  Dependencies:");
        let mut has_dependencies = false;
        for unit in deployment_order {
            if !unit.config.dependencies.is_empty() {
                has_dependencies = true;
                for dep in &unit.config.dependencies {
                    let (dep_display, dep_unit_name) = if let Some(name) = dep.name() {
                        (name.clone(), name.clone())
                    } else if let Some(path) = dep.path() {
                        let (service, _) = self.extract_service_module_from_path(path)?;
                        (path.clone(), service)
                    } else {
                        continue;
                    };
                    
                    let environment_to_use = self.resolve_dependency_environment(&dep_unit_name, environment_overrides, env_id)?;
                    let resolved_env = environment_resolver.resolve_environment(&environment_to_use)?;
                    self.output_manager.print_msg(&format!("    {} → {} ({})", dep_display, environment_to_use, resolved_env.workspace));
                }
            }
        }
        if !has_dependencies {
            self.output_manager.print_msg("    (none)");
        }
        self.output_manager.print_msg("");

        self.output_manager.print_bold("  Deployment Order:");
        for (index, unit) in deployment_order.iter().enumerate() {
            self.output_manager.print_msg(&format!("    {}. {} ({:?})",
                index + 1,
                unit.config.name,
                unit.config.unit_type
            ));
            self.output_manager.print_msg(&format!("       Path:  {}", unit.path.display()));
            self.output_manager.print_msg(&format!("       State: {:?}", unit.config.state_management));
            self.output_manager.print_msg("");
        }

        self.output_manager.print_bold("  Summary:");
        self.output_manager.print_msg(&format!("    Total units: {}", deployment_order.len()));

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

    fn extract_service_module_from_path(&self, path: &str) -> Result<(String, String)> {
        // Convert source path to service/module
        // e.g., "../database/modules/dynamodb" -> ("database", "dynamodb")
        // e.g., "./lambda" -> (current_service, "lambda")
        // e.g., "../../database/modules/dynamodb" -> ("database", "dynamodb")

        let normalized_source = path
            .replace("../", "")
            .replace("./", "")
            .replace("//", "/");

        let parts: Vec<&str> = normalized_source.split('/').collect();

        if parts.len() >= 2 {
            let service = parts[0].to_string();
            let module = parts[parts.len() - 1].to_string();
            Ok((service, module))
        } else if parts.len() == 1 {
            // Local module reference
            Ok(("current".to_string(), parts[0].to_string()))
        } else {
            Err(EnvieError::ValidationError(
                format!("Invalid source path: {}", path)
            ))
        }
    }
    
    fn convert_unit_to_module_config(&self, unit: &DiscoveredUnit, environment_overrides: &HashMap<String, String>, default_env: &str, unit_registry: &UnitRegistry) -> ModuleConfig {
        // Convert UnitConfig to ModuleConfig for backward compatibility with TerraformGenerator
        ModuleConfig {
            name: unit.config.name.clone(),
            description: unit.config.description.clone(),
            path: unit.path.to_string_lossy().to_string(),
            dependencies: unit.config.dependencies.iter().map(|dep| {
                let (dep_path, dep_unit_name) = if let Some(name) = dep.name() {
                    if let Some(dep_unit) = unit_registry.get_unit(name) {
                        (dep_unit.path.to_string_lossy().to_string(), name.clone())
                    } else {
                        return crate::common::service_config::DependencyReference {
                            path: format!("units/{}", name),
                            environment: default_env.to_string(),
                        };
                    }
                } else if let Some(path) = dep.path() {
                    let (service, _) = self.extract_service_module_from_path(path).unwrap_or(("unknown".to_string(), String::new()));
                    (path.clone(), service)
                } else {
                    return crate::common::service_config::DependencyReference {
                        path: "unknown".to_string(),
                        environment: default_env.to_string(),
                    };
                };

                let environment = self.resolve_dependency_environment(&dep_unit_name, environment_overrides, default_env)
                    .unwrap_or_else(|_| default_env.to_string());

                crate::common::service_config::DependencyReference {
                    path: dep_path,
                    environment,
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
    
    fn validate_env_id(&self, env_id: &str) -> Result<()> {
        // Allow alphanumeric characters, hyphens, underscores, and dots
        // Prevent path traversal (../, ..\, etc.) and other dangerous characters
        let re = Regex::new(r"^[0-9A-Za-z][0-9A-Za-z._-]*$")?;
        
        if !re.is_match(env_id) {
            return Err(EnvieError::ValidationError(
                format!("Invalid environment ID '{}'. Environment IDs must contain only alphanumeric characters, hyphens, underscores, and dots. Path traversal characters (../, ..\\) are not allowed.", env_id)
            ));
        }
        
        // Additional check: explicitly reject path traversal patterns
        if env_id.contains("..") || env_id.contains("/") || env_id.contains("\\") {
            return Err(EnvieError::ValidationError(
                format!("Invalid environment ID '{}'. Path traversal characters are not allowed.", env_id)
            ));
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
    
    fn load_environment_config(&self) -> Result<EnvironmentConfig> {
        let workspace_file = self.working_directory.join("workspace.envie.yaml");
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
    
    /// Find the project root by searching up for workspace.envie.yaml
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

