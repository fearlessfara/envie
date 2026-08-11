//! The shape of `workspace.envie.yaml`.
//!
//! Unknown keys are ignored, so a file written by an earlier version — which
//! also listed `services:` and `defaults:` — still loads.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub version: String,

    #[serde(default)]
    pub project: Option<ProjectInfo>,

    #[serde(default)]
    pub environments: Option<crate::common::environment::EnvironmentConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_workspace_file_parses() {
        let config: WorkspaceConfig = serde_yaml::from_str(
            r#"
version: "1.0"
project:
  name: my-project
  description: Multi-unit Terraform monorepo
"#,
        )
        .unwrap();

        assert_eq!(config.version, "1.0");
        assert_eq!(config.project.unwrap().name, "my-project");
    }

    /// Files written before the unit layout replaced services and modules must
    /// keep loading, since the keys they carry are simply no longer read.
    #[test]
    fn keys_from_older_versions_are_ignored_rather_than_rejected() {
        let config: WorkspaceConfig = serde_yaml::from_str(
            r#"
version: "1.0"
project:
  name: my-project
services:
  - path: services/api
  - path: services/database
defaults:
  region: eu-west-1
"#,
        )
        .unwrap();

        assert_eq!(config.version, "1.0");
    }
}
