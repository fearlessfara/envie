use crate::common::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct DiscoveredService {
    pub path: PathBuf,
    pub config: ServiceConfig,
    pub modules: Vec<DiscoveredModule>,
}

#[derive(Debug, Clone)]
pub struct DiscoveredModule {
    pub path: PathBuf,
    pub config: ModuleConfig,
}

#[derive(Debug, Clone)]
pub struct ServiceRegistry {
    pub services: HashMap<String, DiscoveredService>,
    pub modules: HashMap<String, DiscoveredModule>,
}

impl ServiceRegistry {
    pub fn discover_from_path<P: AsRef<Path>>(root_path: P) -> Result<Self> {
        let mut services = HashMap::new();
        let mut modules = HashMap::new();
        
        let root_path = root_path.as_ref();
        
        // Look for workspace.envie or .envie.yaml at root
        let workspace_config = Self::find_workspace_config(root_path)?;
        
        // Discover services based on workspace config or auto-discovery
        let service_paths = if let Some(config) = &workspace_config {
            // Use explicit service paths from workspace config
            config.services.iter()
                .map(|s| root_path.join(&s.path))
                .collect()
        } else {
            // Auto-discover: look for directories with .envie files
            Self::auto_discover_services(root_path)?
        };
        
        for service_path in service_paths {
            if let Ok(service) = Self::discover_service(&service_path) {
                let service_name = service.config.name.clone();
                services.insert(service_name.clone(), service.clone());
                
                // Register modules
                for module in &service.modules {
                    let module_name = format!("{}/{}", service_name, module.config.name);
                    modules.insert(module_name, module.clone());
                }
            }
        }
        
        Ok(ServiceRegistry { services, modules })
    }
    
    fn find_workspace_config<P: AsRef<Path>>(root_path: P) -> Result<Option<WorkspaceConfig>> {
        let root_path = root_path.as_ref();
        
        // Try workspace.envie first
        let workspace_envie = root_path.join("workspace.envie");
        if workspace_envie.exists() {
            return Ok(Some(WorkspaceConfig::from_file(workspace_envie)?));
        }
        
        // Try .envie.yaml as fallback
        let envie_yaml = root_path.join(".envie.yaml");
        if envie_yaml.exists() {
            return Ok(Some(WorkspaceConfig::from_file(envie_yaml)?));
        }
        
        Ok(None)
    }
    
    fn auto_discover_services<P: AsRef<Path>>(root_path: P) -> Result<Vec<PathBuf>> {
        let mut service_paths = Vec::new();
        
        for entry in WalkDir::new(root_path)
            .max_depth(3)  // Don't go too deep
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_name() == ".envie" {
                if let Some(parent) = entry.path().parent() {
                    service_paths.push(parent.to_path_buf());
                }
            }
        }
        
        Ok(service_paths)
    }
    
    fn discover_service<P: AsRef<Path>>(service_path: P) -> Result<DiscoveredService> {
        let service_path = service_path.as_ref();
        let config_path = service_path.join(".envie");
        
        if !config_path.exists() {
            return Err(EnvieError::ConfigError(
                format!("No .envie file found in {}", service_path.display())
            ));
        }
        
        let config = ServiceConfig::from_file(config_path)?;
        let mut modules = Vec::new();
        
        // Discover modules within this service
        for module_config in &config.modules {
            let module_path = if module_config.path.is_empty() {
                service_path.join("modules").join(&module_config.name)
            } else {
                service_path.join(&module_config.path)
            };
            
            // Look for module-specific .envie file
            let module_envie_path = module_path.join(".envie");
            let module_config = if module_envie_path.exists() {
                ModuleConfig::from_file(module_envie_path)?
            } else {
                module_config.clone()
            };
            
            modules.push(DiscoveredModule {
                path: module_path,
                config: module_config,
            });
        }
        
        Ok(DiscoveredService {
            path: service_path.to_path_buf(),
            config,
            modules,
        })
    }
    
    pub fn find_service_by_path<P: AsRef<Path>>(&self, path: P) -> Option<&DiscoveredService> {
        let path = path.as_ref();
        
        // Try exact path match first
        for service in self.services.values() {
            if service.path == path {
                return Some(service);
            }
        }
        
        // Try parent directory match (for modules)
        for service in self.services.values() {
            if path.starts_with(&service.path) {
                return Some(service);
            }
        }
        
        None
    }
    
    pub fn find_module_by_path<P: AsRef<Path>>(&self, path: P) -> Option<&DiscoveredModule> {
        let path = path.as_ref();
        
        for module in self.modules.values() {
            if module.path == path {
                return Some(module);
            }
        }
        
        None
    }
    
    pub fn resolve_dependencies(&self, service_name: &str) -> Result<Vec<String>> {
        let mut visited = std::collections::HashSet::new();
        let mut recursion_stack = std::collections::HashSet::new();
        let mut deployment_order = Vec::new();
        
        if let Some(service) = self.services.get(service_name) {
            self.resolve_service_dependencies_recursive(
                service,
                &mut visited,
                &mut recursion_stack,
                &mut deployment_order,
            )?;
        } else {
            return Err(EnvieError::ValidationError(
                format!("Service '{}' not found", service_name)
            ));
        }
        
        Ok(deployment_order)
    }
    
    fn resolve_service_dependencies_recursive(
        &self,
        service: &DiscoveredService,
        visited: &mut std::collections::HashSet<String>,
        recursion_stack: &mut std::collections::HashSet<String>,
        deployment_order: &mut Vec<String>,
    ) -> Result<()> {
        // Check for cyclic dependencies
        if recursion_stack.contains(&service.config.name) {
            return Err(EnvieError::DependencyError(
                format!("Cyclic dependency detected involving service {}", service.config.name)
            ));
        }
        
        // Skip if already visited
        if visited.contains(&service.config.name) {
            return Ok(());
        }
        
        // Mark as visited and add to recursion stack
        visited.insert(service.config.name.clone());
        recursion_stack.insert(service.config.name.clone());
        
        // Resolve service dependencies first
        for dep_path in &service.config.depends {
            let dep_name = self.resolve_dependency_name(dep_path, &service.path)?;
            if let Some(dep_service) = self.services.get(&dep_name) {
                self.resolve_service_dependencies_recursive(
                    dep_service,
                    visited,
                    recursion_stack,
                    deployment_order,
                )?;
            }
        }
        
        // Remove from recursion stack and add to deployment order
        recursion_stack.remove(&service.config.name);
        deployment_order.push(service.config.name.clone());
        
        Ok(())
    }
    
    fn resolve_dependency_name(&self, dep_path: &str, current_path: &Path) -> Result<String> {
        if dep_path.starts_with("../") {
            // For relative paths like "../networking", we need to find the service
            // by looking at the directory name after resolving the path
            let resolved_path = current_path.parent()
                .ok_or_else(|| EnvieError::ValidationError("Invalid relative path".to_string()))?
                .join(dep_path);
            
            // Extract the service name from the path (last component)
            let service_name = resolved_path.file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| EnvieError::ValidationError("Invalid service name in path".to_string()))?;
            
            // Check if this service exists
            if self.services.contains_key(service_name) {
                return Ok(service_name.to_string());
            }
            
            Err(EnvieError::ValidationError(
                format!("Dependency '{}' not found - service '{}' does not exist", dep_path, service_name)
            ))
        } else if dep_path.contains("/") {
            // service/module format
            Ok(dep_path.to_string())
        } else {
            // Just service name
            Ok(dep_path.to_string())
        }
    }
    
    fn normalize_path(&self, path: &Path) -> PathBuf {
        // Use the standard library's path normalization
        let mut normalized = PathBuf::new();
        
        for component in path.components() {
            match component {
                std::path::Component::ParentDir => {
                    if !normalized.pop() {
                        // If we can't go up, just ignore the .. component
                        // This prevents removing too many components
                    }
                }
                std::path::Component::CurDir => {
                    // Skip current directory
                }
                _ => {
                    normalized.push(component);
                }
            }
        }
        
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_service_discovery() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        
        // Create a simple service structure
        let api_dir = root.join("services").join("api");
        fs::create_dir_all(&api_dir).unwrap();
        
        let api_config = r#"
name: api
description: API service
modules:
  - name: lambda
    path: modules/lambda
depends:
  - ../database
"#;
        
        fs::write(api_dir.join(".envie"), api_config).unwrap();
        
        // Create lambda module
        let lambda_dir = api_dir.join("modules").join("lambda");
        fs::create_dir_all(&lambda_dir).unwrap();
        
        let lambda_config = r#"
name: lambda
description: Lambda function
remote_states:
  - name: db
    source: ../../database/modules/dynamodb
    outputs: [table_name]
"#;
        
        fs::write(lambda_dir.join(".envie"), lambda_config).unwrap();
        
        // Discover services
        let registry = ServiceRegistry::discover_from_path(root).unwrap();
        
        assert!(registry.services.contains_key("api"));
        assert!(registry.modules.contains_key("api/lambda"));
        
        let api_service = &registry.services["api"];
        assert_eq!(api_service.config.name, "api");
        assert_eq!(api_service.config.depends.len(), 1);
        assert!(api_service.config.depends.contains(&"../database".to_string()));
    }
}
