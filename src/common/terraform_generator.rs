use crate::common::*;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct TerraformGenerator {
    pub backend_config: BackendConfig,
}

#[derive(Debug, Clone)]
pub struct BackendConfig {
    pub backend_type: String,
    pub config: HashMap<String, String>,
}

impl Default for BackendConfig {
    fn default() -> Self {
        let mut config = HashMap::new();
        config.insert("bucket".to_string(), "terraform-state-bucket".to_string());
        config.insert("region".to_string(), "eu-west-1".to_string());
        config.insert("key".to_string(), "terraform.tfstate".to_string());
        
        Self {
            backend_type: "s3".to_string(),
            config,
        }
    }
}

impl TerraformGenerator {
    pub fn new() -> Self {
        Self {
            backend_config: BackendConfig::default(),
        }
    }
    
    pub fn with_backend(mut self, backend: BackendConfig) -> Self {
        self.backend_config = backend;
        self
    }
    
    pub fn generate_remote_state_data_sources(
        &self,
        module_path: &Path,
        dependencies: &[crate::common::service_config::DependencyReference],
        environment_resolver: &EnvironmentResolver,
        environment_overrides: &std::collections::HashMap<String, String>,
        _service_name: &str,
        _module_name: &str,
    ) -> Result<String> {
        let mut generated = String::new();

        // Generate remote state data sources for each dependency
        // Note: We don't scan for existing data sources because this file is auto-generated
        // and gets completely regenerated each time
        for dependency in dependencies {
            // Check for CLI override for this dependency
            let (source_service, source_module) = self.extract_service_module_from_source(&dependency.path)?;

            let environment_to_use = if let Some(override_env) = environment_overrides.get(&source_service) {
                override_env.clone()
            } else {
                dependency.environment.clone()
            };

            let resolved_env = environment_resolver.resolve_environment(&environment_to_use)?;

            // Generate data source name from the dependency unit name
            // For path "../core" this will be "core"
            // For path "../db/submodule" this will be "db_submodule"
            let data_source_name = if source_module.is_empty() {
                source_service.to_string()
            } else {
                format!("{}_{}", source_service, source_module)
            };

            // Generate state key path (just the path portion, without environment prefix)
            let state_key_suffix = if source_module.is_empty() {
                format!("{}/{}/terraform.tfstate", source_service, source_service)
            } else {
                format!("{}/{}/terraform.tfstate", source_service, source_module)
            };

            // Use variables for backend configuration
            // Choose the correct attribute based on backend type
            let state_attr = if resolved_env.backend.backend_type == "local" {
                "path"
            } else {
                "key"
            };
            
            generated.push_str(&format!(
                r#"data "terraform_remote_state" "{}" {{
  backend = "{}"
  workspace = var.envie_workspace

  config = {{
    {}            = "ephemeral/${{var.envie_workspace}}/{}"
"#,
                data_source_name,
                resolved_env.backend.backend_type,
                state_attr,
                state_key_suffix
            ));

            // Add other backend config using variables
            for (key, _) in &resolved_env.backend.config {
                if key == "key_pattern" || key == "key" {
                    continue; // Already handled above
                }
                generated.push_str(&format!("    {} = var.envie_backend_{}\n", key, key));
            }

            generated.push_str("  }\n}\n\n");
        }
        
        Ok(generated)
    }
    
    fn extract_service_module_from_source(&self, source: &str) -> Result<(String, String)> {
        // Convert source path to unit name
        // e.g., "../core" -> ("core", "")
        // e.g., "../database/modules/dynamodb" -> ("database", "dynamodb")
        // e.g., "./lambda" -> ("current", "lambda")
        // e.g., "units/api" -> ("api", "")  (handle full paths from name-based deps)
        // e.g., "units/core" -> ("core", "")

        let normalized_source = source
            .replace("../", "")
            .replace("./", "")
            .replace("//", "/");

        let parts: Vec<&str> = normalized_source.split('/').filter(|s| !s.is_empty()).collect();

        // Handle full paths like "units/api" or "units/core" (from name-based dependencies)
        if parts.len() >= 2 && parts[0] == "units" {
            // Path like "units/api" -> extract "api" as the unit name
            if parts.len() == 2 {
                Ok((parts[1].to_string(), String::new()))
            } else {
                // Path like "units/database/modules/dynamodb" -> ("database", "dynamodb")
                Ok((parts[1].to_string(), parts[parts.len() - 1].to_string()))
            }
        } else if parts.len() >= 2 {
            // Multi-part path: first part is unit, last part is sub-module
            let unit = parts[0].to_string();
            let module = parts[parts.len() - 1].to_string();
            Ok((unit, module))
        } else if parts.len() == 1 {
            // Single part path
            if source.starts_with("./") {
                // ./something is a local module
                Ok(("current".to_string(), parts[0].to_string()))
            } else {
                // ../something is a unit reference with no sub-module
                Ok((parts[0].to_string(), String::new()))
            }
        } else {
            Err(EnvieError::ValidationError(
                format!("Invalid source path: {}", source)
            ))
        }
    }
    
    fn generate_state_key(&self, source: &str, _workspace: &str) -> Result<String> {
        // Convert source path to state key
        // e.g., "../database/modules/dynamodb" -> "database/dynamodb/terraform.tfstate"
        let normalized_source = source
            .replace("../", "")
            .replace("./", "")
            .replace("//", "/");
        
        Ok(format!("{}/terraform.tfstate", normalized_source))
    }
    
    pub fn generate_backend_configuration(
        &self,
        module_config: &ModuleConfig,
        environment_resolver: &EnvironmentResolver,
        service_name: &str,
        module_name: &str,
    ) -> Result<String> {
        // Determine the environment to use (for now, use ephemeral as default)
        let environment = "ephemeral";
        let resolved_env = environment_resolver.resolve_environment(environment)?;

        // Generate state key pattern based on state management strategy
        let state_key_suffix = match &module_config.state_management {
            crate::common::service_config::StateManagement::Dedicated => {
                // Module gets its own state file
                let module_part = if module_name.is_empty() { service_name } else { module_name };
                format!("{}/{}/terraform.tfstate", service_name, module_part)
            }
            crate::common::service_config::StateManagement::Service => {
                // Module shares service-level state
                format!("{}/service/terraform.tfstate", service_name)
            }
            crate::common::service_config::StateManagement::Shared(shared_id) => {
                // Module shares state with other modules
                format!("{}/shared/terraform.tfstate", shared_id)
            }
        };

        // Terraform doesn't support variables in backend configuration
        // We'll create a minimal backend block and use -backend-config during init
        Ok(format!(r#"# State management: {:?}
# State key pattern: ephemeral/${{workspace}}/{}
# Note: Backend values are provided via -backend-config during terraform init

terraform {{
  backend "{}" {{
  }}
}}
"#,
            module_config.state_management,
            state_key_suffix,
            resolved_env.backend.backend_type
        ))
    }
    
    pub fn generate_locals_file(
        &self,
        project_name: &str,
        environment_id: &str,
        unit_name: &str,
    ) -> String {
        format!(r#"locals {{
  envie_project_name   = "{}"
  envie_environment_id = "{}"
  envie_unit_name      = "{}"

  # Common tags provided by Envie
  envie_common_tags = {{
    Project     = "{}"
    Environment = "{}"
    Unit        = "{}"
    ManagedBy   = "envie"
  }}
}}
"#, project_name, environment_id, unit_name, project_name, environment_id, unit_name)
    }

    pub fn generate_variables_file(
        &self,
        environment_resolver: &EnvironmentResolver,
    ) -> Result<String> {
        // Get the current environment to extract backend config
        let resolved_env = environment_resolver.resolve_environment("ephemeral")?;

        let mut content = String::new();

        // Workspace variable
        content.push_str("variable \"envie_workspace\" {\n");
        content.push_str("  description = \"Terraform workspace name managed by Envie\"\n");
        content.push_str("  type        = string\n");
        content.push_str("}\n\n");

        // Backend configuration variables
        for (key, _) in &resolved_env.backend.config {
            if key == "key_pattern" {
                continue; // Skip key_pattern as we'll handle state keys differently
            }

            let var_name = format!("envie_backend_{}", key);
            let description = match key.as_str() {
                "bucket" => "S3 bucket for Terraform state",
                "region" => "AWS region for Terraform state backend",
                "dynamodb_table" => "DynamoDB table for state locking",
                _ => "Backend configuration value",
            };

            content.push_str(&format!("variable \"{}\" {{\n", var_name));
            content.push_str(&format!("  description = \"{}\"\n", description));
            content.push_str("  type        = string\n");
            content.push_str("}\n\n");
        }

        Ok(content)
    }

    pub fn write_generated_files(
        &self,
        module_path: &Path,
        dependencies: &[crate::common::service_config::DependencyReference],
        module_config: &ModuleConfig,
        environment_resolver: &EnvironmentResolver,
        environment_overrides: &std::collections::HashMap<String, String>,
        service_name: &str,
        module_name: &str,
        project_name: &str,
        environment_id: &str,
    ) -> Result<()> {
        // Generate all content sections
        let backend_content = self.generate_backend_configuration(
            module_config,
            environment_resolver,
            service_name,
            module_name,
        )?;
        
        let remote_state_content = self.generate_remote_state_data_sources(
            module_path,
            dependencies,
            environment_resolver,
            environment_overrides,
            service_name,
            module_name,
        )?;

        let locals_content = self.generate_locals_file(project_name, environment_id, module_name);

        let variables_content = self.generate_variables_file(environment_resolver)?;

        // Combine all into a single config file
        let mut config_content = String::new();
        config_content.push_str("# Auto-generated by Envie - DO NOT EDIT\n");
        config_content.push_str("# This file is automatically generated and will be overwritten\n\n");
        
        // Add variables section
        config_content.push_str("# ============================================================================\n");
        config_content.push_str("# Variables\n");
        config_content.push_str("# ============================================================================\n\n");
        config_content.push_str(&variables_content);
        config_content.push_str("\n");
        
        // Add locals section
        config_content.push_str("# ============================================================================\n");
        config_content.push_str("# Locals\n");
        config_content.push_str("# ============================================================================\n\n");
        config_content.push_str(&locals_content);
        config_content.push_str("\n");
        
        // Add backend configuration section
        config_content.push_str("# ============================================================================\n");
        config_content.push_str("# Backend Configuration\n");
        config_content.push_str("# ============================================================================\n\n");
        config_content.push_str(&backend_content);
        config_content.push_str("\n");
        
        // Add remote state data sources section
        config_content.push_str("# ============================================================================\n");
        config_content.push_str("# Remote State Data Sources\n");
        config_content.push_str("# ============================================================================\n\n");
        config_content.push_str(&remote_state_content);

        // Write single config file
        let config_file = module_path.join("config.envie.tf");
        std::fs::write(config_file, config_content)?;

        // Remove old individual files if they exist (cleanup)
        let _ = std::fs::remove_file(module_path.join("backend.envie.tf"));
        let _ = std::fs::remove_file(module_path.join("remote_state.envie.tf"));
        let _ = std::fs::remove_file(module_path.join("locals.envie.tf"));
        let _ = std::fs::remove_file(module_path.join("variables.envie.tf"));

        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_remote_state_generation() {
        let generator = TerraformGenerator::new();
        let temp_dir = TempDir::new().unwrap();
        let module_path = temp_dir.path();
        
        let remote_states = vec![
            RemoteStateReference {
                name: "database".to_string(),
                source: "../database/modules/dynamodb".to_string(),
                workspace: Some("sandbox".to_string()),
                outputs: vec!["table_name".to_string(), "table_arn".to_string()],
            }
        ];
        
        let workspace_resolver = WorkspaceResolver::new(
            "myapp-123".to_string(),
            ServiceRegistry {
                services: HashMap::new(),
                modules: HashMap::new(),
            }
        );
        
        let generated = generator.generate_remote_state_data_sources(
            module_path,
            &remote_states,
            &workspace_resolver,
        ).unwrap();
        
        assert!(generated.contains("data \"terraform_remote_state\" \"database\""));
        assert!(generated.contains("workspace = \"sandbox\""));
        assert!(generated.contains("key = \"database/modules/dynamodb/terraform.tfstate\""));
    }
    
    #[test]
    fn test_module_variables_generation() {
        let generator = TerraformGenerator::new();
        
        let mut variables = HashMap::new();
        variables.insert("runtime".to_string(), serde_json::Value::String("nodejs18.x".to_string()));
        variables.insert("timeout".to_string(), serde_json::Value::Number(serde_json::Number::from(30)));
        variables.insert("memory".to_string(), serde_json::Value::Number(serde_json::Number::from(512)));
        
        let module_config = ModuleConfig {
            name: "lambda".to_string(),
            description: "Lambda function".to_string(),
            path: "modules/lambda".to_string(),
            dependencies: vec![],
            remote_states: vec![],
            variables,
        };
        
        let generated = generator.generate_module_variables(&module_config).unwrap();
        
        assert!(generated.contains("variable \"runtime\""));
        assert!(generated.contains("default = \"nodejs18.x\""));
        assert!(generated.contains("variable \"timeout\""));
        assert!(generated.contains("default = 30"));
    }
}
