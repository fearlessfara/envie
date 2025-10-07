use crate::common::*;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::common::service_config::ProjectInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentConfig {
    #[serde(default)]
    pub project: Option<ProjectInfo>,
    pub ephemeral: EphemeralConfig,
    pub stable: HashMap<String, StableEnvironmentConfig>,
}

// Use the ProjectInfo from service_config module

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EphemeralConfig {
    pub naming_pattern: String,
    pub backend: BackendConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StableEnvironmentConfig {
    pub workspace: String,
    pub backend: BackendConfig,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendConfig {
    #[serde(rename = "type")]
    pub backend_type: String,
    pub config: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub enum EnvironmentType {
    Ephemeral,
    Stable(String),
}

#[derive(Debug, Clone)]
pub struct ResolvedEnvironment {
    pub workspace: String,
    pub environment_type: EnvironmentType,
    pub backend: BackendConfig,
}

#[derive(Debug, Clone)]
pub struct EnvironmentResolver {
    pub current_workspace: String,
    pub project_name: String,
    pub available_workspaces: Vec<String>,
    pub environment_config: EnvironmentConfig,
}

impl EnvironmentResolver {
    pub fn new(
        current_workspace: String,
        project_name: String,
        environment_config: EnvironmentConfig,
    ) -> Self {
        Self {
            current_workspace,
            project_name,
            available_workspaces: Vec::new(),
            environment_config,
        }
    }
    
    pub fn with_available_workspaces(mut self, workspaces: Vec<String>) -> Self {
        self.available_workspaces = workspaces;
        self
    }
    
    pub fn resolve_environment(&self, env_ref: &str) -> Result<ResolvedEnvironment> {
        if env_ref.starts_with("stable.") {
            // stable.sandbox → sandbox
            let env_name = env_ref.strip_prefix("stable.").unwrap();
            self.resolve_stable_environment(env_name)
        } else if env_ref == "ephemeral" {
            // ephemeral → current MR
            self.resolve_current_ephemeral()
        } else if env_ref.starts_with("ephemeral.") {
            // ephemeral.123 → myapp-123
            let mr_number = env_ref.strip_prefix("ephemeral.").unwrap();
            self.resolve_specific_ephemeral(mr_number)
        } else {
            // Direct workspace reference (myapp-123, myapp-feature-auth)
            self.resolve_direct_workspace(env_ref)
        }
    }
    
    fn resolve_stable_environment(&self, env_name: &str) -> Result<ResolvedEnvironment> {
        let stable_env = self.environment_config.stable.get(env_name)
            .ok_or_else(|| EnvieError::ValidationError(
                format!("Stable environment '{}' not found. Available: {:?}", 
                    env_name, 
                    self.environment_config.stable.keys().collect::<Vec<_>>())
            ))?;
        
        Ok(ResolvedEnvironment {
            workspace: stable_env.workspace.clone(),
            environment_type: EnvironmentType::Stable(env_name.to_string()),
            backend: stable_env.backend.clone(),
        })
    }
    
    fn resolve_current_ephemeral(&self) -> Result<ResolvedEnvironment> {
        Ok(ResolvedEnvironment {
            workspace: self.current_workspace.clone(),
            environment_type: EnvironmentType::Ephemeral,
            backend: self.environment_config.ephemeral.backend.clone(),
        })
    }
    
    fn resolve_specific_ephemeral(&self, id: &str) -> Result<ResolvedEnvironment> {
        let workspace = format!("{}-{}", self.project_name, id);
        
        // Validate workspace exists
        if !self.available_workspaces.contains(&workspace) {
            return Err(EnvieError::ValidationError(
                format!("Ephemeral workspace '{}' does not exist. Available: {:?}", 
                    workspace, self.available_workspaces)
            ));
        }
        
        Ok(ResolvedEnvironment {
            workspace,
            environment_type: EnvironmentType::Ephemeral,
            backend: self.environment_config.ephemeral.backend.clone(),
        })
    }
    
    fn resolve_direct_workspace(&self, workspace: &str) -> Result<ResolvedEnvironment> {
        // Try to detect if it's an ephemeral or stable workspace
        let environment_type = if workspace.starts_with(&format!("{}-", self.project_name)) {
            EnvironmentType::Ephemeral
        } else {
            // Assume it's a stable workspace
            EnvironmentType::Stable(workspace.to_string())
        };
        
        // Determine backend based on environment type
        let backend = match &environment_type {
            EnvironmentType::Ephemeral => self.environment_config.ephemeral.backend.clone(),
            EnvironmentType::Stable(env_name) => {
                self.environment_config.stable.get(env_name)
                    .map(|env| env.backend.clone())
                    .unwrap_or_else(|| self.environment_config.ephemeral.backend.clone())
            }
        };
        
        Ok(ResolvedEnvironment {
            workspace: workspace.to_string(),
            environment_type,
            backend,
        })
    }
    
    pub fn generate_state_key(&self, resolved_env: &ResolvedEnvironment, service: &str, module: &str) -> String {
        match &resolved_env.environment_type {
            EnvironmentType::Ephemeral => {
                // ephemeral/{workspace}/{service}/{module}/terraform.tfstate
                format!("ephemeral/{}/{}/{}/terraform.tfstate", 
                    resolved_env.workspace, service, module)
            }
            EnvironmentType::Stable(env_name) => {
                // Use the key_pattern from the backend config and substitute placeholders
                let default_pattern = "stable/{environment}/{service}/{module}/terraform.tfstate".to_string();
                let key_pattern = resolved_env.backend.config.get("key_pattern")
                    .unwrap_or(&default_pattern);
                
                key_pattern
                    .replace("{environment}", env_name)
                    .replace("{service}", service)
                    .replace("{module}", module)
            }
        }
    }
    
    pub fn generate_backend_config(&self, resolved_env: &ResolvedEnvironment, service: &str, module: &str) -> String {
        let state_key = self.generate_state_key(resolved_env, service, module);
        
        let mut config_items = String::new();
        for (key, value) in &resolved_env.backend.config {
            if key == "key" {
                config_items.push_str(&format!("    {} = \"{}\"\n", key, state_key));
            } else {
                config_items.push_str(&format!("    {} = \"{}\"\n", key, value));
            }
        }
        
        format!(r#"terraform {{
  backend "{}" {{
{}
  }}
}}
"#,
            resolved_env.backend.backend_type,
            config_items
        )
    }
}

impl EnvironmentConfig {
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: EnvironmentConfig = serde_yaml::from_str(&content)
            .map_err(|e| EnvieError::ConfigError(format!("Failed to parse environment config: {}", e)))?;
        Ok(config)
    }
    
    pub fn from_str(content: &str) -> Result<Self> {
        let config: EnvironmentConfig = serde_yaml::from_str(content)
            .map_err(|e| EnvieError::ConfigError(format!("Failed to parse environment config: {}", e)))?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_environment_resolution() {
        let mut stable_envs = HashMap::new();
        stable_envs.insert("sandbox".to_string(), StableEnvironmentConfig {
            workspace: "sandbox".to_string(),
            backend: BackendConfig {
                backend_type: "s3".to_string(),
                config: {
                    let mut config = HashMap::new();
                    config.insert("bucket".to_string(), "terraform-state-stable".to_string());
                    config.insert("region".to_string(), "eu-west-1".to_string());
                    config
                },
            },
            description: "Sandbox environment".to_string(),
        });
        
        let environment_config = EnvironmentConfig {
            ephemeral: EphemeralConfig {
                naming_pattern: "{repo}-{merge-request}".to_string(),
                backend: BackendConfig {
                    backend_type: "s3".to_string(),
                    config: {
                        let mut config = HashMap::new();
                        config.insert("bucket".to_string(), "terraform-state-ephemeral".to_string());
                        config.insert("region".to_string(), "eu-west-1".to_string());
                        config
                    },
                },
            },
            stable: stable_envs,
        };
        
        let resolver = EnvironmentResolver::new(
            "myapp-123".to_string(),
            "myapp".to_string(),
            environment_config,
        ).with_available_workspaces(vec!["myapp-123".to_string(), "myapp-456".to_string()]);
        
        // Test stable environment resolution
        let stable_result = resolver.resolve_environment("stable.sandbox").unwrap();
        assert_eq!(stable_result.workspace, "sandbox");
        assert!(matches!(stable_result.environment_type, EnvironmentType::Stable(name) if name == "sandbox"));
        
        // Test current ephemeral resolution
        let ephemeral_result = resolver.resolve_environment("ephemeral").unwrap();
        assert_eq!(ephemeral_result.workspace, "myapp-123");
        assert!(matches!(ephemeral_result.environment_type, EnvironmentType::Ephemeral));
        
        // Test specific ephemeral resolution
        let specific_ephemeral = resolver.resolve_environment("ephemeral.456").unwrap();
        assert_eq!(specific_ephemeral.workspace, "myapp-456");
        assert!(matches!(specific_ephemeral.environment_type, EnvironmentType::Ephemeral));
    }
    
    #[test]
    fn test_state_key_generation() {
        let environment_config = EnvironmentConfig {
            ephemeral: EphemeralConfig {
                naming_pattern: "{repo}-{merge-request}".to_string(),
                backend: BackendConfig {
                    backend_type: "s3".to_string(),
                    config: HashMap::new(),
                },
            },
            stable: HashMap::new(),
        };
        
        let resolver = EnvironmentResolver::new(
            "myapp-123".to_string(),
            "myapp".to_string(),
            environment_config,
        );
        
        let ephemeral_env = ResolvedEnvironment {
            workspace: "myapp-123".to_string(),
            environment_type: EnvironmentType::Ephemeral,
            backend: BackendConfig {
                backend_type: "s3".to_string(),
                config: HashMap::new(),
            },
        };
        
        let state_key = resolver.generate_state_key(&ephemeral_env, "api", "lambda");
        assert_eq!(state_key, "ephemeral/myapp-123/api/lambda/terraform.tfstate");
    }
}
