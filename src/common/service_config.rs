use crate::common::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    
    #[serde(default)]
    pub description: String,
    
    #[serde(default)]
    pub modules: Vec<ModuleConfig>,
    
    #[serde(default)]
    pub depends: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleConfig {
    pub name: String,
    
    #[serde(default)]
    pub description: String,
    
    #[serde(default)]
    pub path: String,
    
    #[serde(default)]
    pub depends: Vec<DependencyReference>,
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

impl ServiceConfig {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: ServiceConfig = serde_yaml::from_str(&content)
            .map_err(|e| crate::common::EnvieError::ConfigError(format!("Failed to parse service config: {}", e)))?;
        Ok(config)
    }
    
    pub fn from_str(content: &str) -> Result<Self> {
        let config: ServiceConfig = serde_yaml::from_str(content)
            .map_err(|e| crate::common::EnvieError::ConfigError(format!("Failed to parse service config: {}", e)))?;
        Ok(config)
    }
}

impl ModuleConfig {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: ModuleConfig = serde_yaml::from_str(&content)
            .map_err(|e| crate::common::EnvieError::ConfigError(format!("Failed to parse module config: {}", e)))?;
        Ok(config)
    }
    
    pub fn from_str(content: &str) -> Result<Self> {
        let config: ModuleConfig = serde_yaml::from_str(content)
            .map_err(|e| crate::common::EnvieError::ConfigError(format!("Failed to parse module config: {}", e)))?;
        Ok(config)
    }
}

impl WorkspaceConfig {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: WorkspaceConfig = serde_yaml::from_str(&content)
            .map_err(|e| crate::common::EnvieError::ConfigError(format!("Failed to parse workspace config: {}", e)))?;
        Ok(config)
    }
    
    pub fn from_str(content: &str) -> Result<Self> {
        let config: WorkspaceConfig = serde_yaml::from_str(content)
            .map_err(|e| crate::common::EnvieError::ConfigError(format!("Failed to parse workspace config: {}", e)))?;
        Ok(config)
    }
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
    depends: []
    remote_states:
      - name: db
        source: ../database/modules/dynamodb
        workspace: sandbox
        outputs: [table_name, table_arn]
  
  - name: gateway
    path: modules/gateway
    depends: [lambda]
    remote_states:
      - name: lambda
        source: ./lambda
        outputs: [function_name, function_arn]

depends:
  - ../database
  - ../networking
"#;

        let config: ServiceConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.name, "api");
        assert_eq!(config.modules.len(), 2);
        assert_eq!(config.depends.len(), 2);
        assert!(config.depends.contains(&"../database".to_string()));
        assert!(config.depends.contains(&"../networking".to_string()));
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
