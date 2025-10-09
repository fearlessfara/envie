use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    
    #[serde(default)]
    pub description: String,
    
    #[serde(default)]
    pub modules: Vec<ModuleConfig>,

    #[serde(default)]
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleConfig {
    pub name: String,
    
    #[serde(default)]
    pub description: String,
    
    #[serde(default)]
    pub path: String,
    
    #[serde(default)]
    pub dependencies: Vec<DependencyReference>,
    
    #[serde(default)]
    pub state_management: StateManagement,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(from = "StateManagementString")]
pub enum StateManagement {
    /// Unit has its own dedicated state file
    Dedicated,
    /// Unit is managed as part of the parent unit's state
    Service,
    /// Unit is managed as part of a shared state with other units
    Shared(String), // The shared state identifier
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum StateManagementString {
    String(String),
    Object { shared_state_id: String },
}

impl From<StateManagementString> for StateManagement {
    fn from(s: StateManagementString) -> Self {
        match s {
            StateManagementString::String(s) => {
                if s == "dedicated" {
                    StateManagement::Dedicated
                } else if s == "service" {
                    StateManagement::Service
                } else if s.starts_with("shared:") {
                    StateManagement::Shared(s.strip_prefix("shared:").unwrap().to_string())
                } else {
                    StateManagement::Service // Default fallback
                }
            }
            StateManagementString::Object { shared_state_id } => {
                StateManagement::Shared(shared_state_id)
            }
        }
    }
}


impl Default for StateManagement {
    fn default() -> Self {
        StateManagement::Service
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyReference {
    pub path: String,  // Path like "../database/modules/dynamodb" or "database.dynamodb"
    pub environment: String,  // stable.sandbox, ephemeral, ephemeral.123, or direct workspace
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub version: String,
    
    #[serde(default)]
    pub project: Option<ProjectInfo>,
    
    #[serde(default)]
    pub services: Vec<ServiceDiscovery>,
    
    #[serde(default)]
    pub defaults: HashMap<String, serde_json::Value>,
    
    #[serde(default)]
    pub environments: Option<crate::common::environment::EnvironmentConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDiscovery {
    pub path: String,
    #[serde(default)]
    pub name: Option<String>,
}




#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_config_parsing() {
        let yaml = r#"
name: api
description: API Gateway and Lambda functions

modules:
  - name: lambda
    path: modules/lambda
    dependencies: []
    remote_states:
      - name: db
        source: ../database/modules/dynamodb
        workspace: sandbox
        outputs: [table_name, table_arn]

  - name: gateway
    path: modules/gateway
    dependencies: [lambda]
    remote_states:
      - name: lambda
        source: ./lambda
        outputs: [function_name, function_arn]

dependencies:
  - ../database
  - ../networking
"#;

        let config: ServiceConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.name, "api");
        assert_eq!(config.modules.len(), 2);
        assert_eq!(config.dependencies.len(), 2);
        assert!(config.dependencies.contains(&"../database".to_string()));
        assert!(config.dependencies.contains(&"../networking".to_string()));
    }

    #[test]
    fn test_workspace_config_parsing() {
        let yaml = r#"
version: "1.0"
project:
  name: my-project
  description: Multi-service Terraform monorepo

services:
  - path: services/api
  - path: services/database
  - path: services/networking

defaults:
  region: eu-west-1
  environment: dev
"#;

        let config: WorkspaceConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.version, "1.0");
        assert_eq!(config.services.len(), 3);
        assert_eq!(config.defaults.get("region").unwrap(), "eu-west-1");
    }
}
