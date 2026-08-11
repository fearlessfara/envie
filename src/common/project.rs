//! Locating and loading the Envie project a command is running inside.

use crate::common::environment::{EnvironmentConfig, EnvironmentResolver};
use crate::common::service_config::WorkspaceConfig;
use crate::common::{EnvieError, Result, UnitDiscovery, UnitRegistry};
use std::path::{Path, PathBuf};

/// Project-level configuration file, at the root of the repository.
pub const WORKSPACE_FILE: &str = "workspace.envie.yaml";

#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub config: WorkspaceConfig,
}

impl Project {
    /// Walk up from `start` looking for the project file.
    pub fn discover(start: &Path) -> Result<Self> {
        let mut current = start.to_path_buf();
        loop {
            let candidate = current.join(WORKSPACE_FILE);
            if candidate.exists() {
                let contents = std::fs::read_to_string(&candidate)?;
                let config: WorkspaceConfig = serde_yaml::from_str(&contents).map_err(|e| {
                    EnvieError::ConfigError(format!("{} is not valid: {}", candidate.display(), e))
                })?;
                return Ok(Self {
                    root: current,
                    config,
                });
            }

            match current.parent() {
                Some(parent) => current = parent.to_path_buf(),
                None => {
                    return Err(EnvieError::ValidationError(format!(
                        "no {} found in {} or any parent directory.\n\
                         If this is an existing Terraform repository, run `envie adopt` to set one up.",
                        WORKSPACE_FILE,
                        start.display()
                    )))
                }
            }
        }
    }

    pub fn name(&self) -> String {
        self.config
            .project
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| {
                self.root
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "envie-project".to_string())
            })
    }

    pub fn environments(&self) -> EnvironmentConfig {
        self.config
            .environments
            .clone()
            .unwrap_or_else(|| EnvironmentConfig {
                project: self.config.project.clone(),
                ephemeral: Default::default(),
                stable: Default::default(),
            })
    }

    /// Build a resolver for a deployment of `environment_id`.
    pub fn resolver(&self, environment_id: &str) -> EnvironmentResolver {
        let environments = self.environments();
        let name = self.name();
        let workspace = environments
            .ephemeral
            .naming_pattern
            .replace("{project}", &name)
            .replace("{id}", environment_id)
            .replace("{env_id}", environment_id);

        EnvironmentResolver::new(workspace, name, environments)
            .with_current_environment_id(environment_id.to_string())
    }

    /// Discover the units declared in this project.
    pub fn units(&self) -> Result<UnitRegistry> {
        let mut discovery = UnitDiscovery::new(self.root.clone());
        discovery.discover_all()?;

        if discovery.registry.units.is_empty() {
            return Err(EnvieError::ValidationError(format!(
                "no deployable units found under {}.\n\
                 Envie looks for envie.yaml files; run `envie adopt` to generate them \
                 from the Terraform root modules already in this repository.",
                self.root.display()
            )));
        }

        Ok(discovery.registry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn project_file(name: &str) -> String {
        format!(
            r#"version: "1.0"
project:
  name: {name}
environments:
  ephemeral:
    naming_pattern: "{{project}}-{{id}}"
    backend:
      type: s3
      config:
        bucket: acme-tfstate
  stable:
    prod:
      workspace: default
      backend:
        type: s3
        config:
          bucket: acme-tfstate
"#
        )
    }

    #[test]
    fn discovery_walks_up_to_the_project_root() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(WORKSPACE_FILE), project_file("acme")).unwrap();
        let nested = tmp.path().join("live/app");
        fs::create_dir_all(&nested).unwrap();

        let project = Project::discover(&nested).unwrap();

        assert_eq!(project.root, tmp.path());
        assert_eq!(project.name(), "acme");
    }

    #[test]
    fn missing_project_file_points_at_adopt() {
        let tmp = TempDir::new().unwrap();

        let error = Project::discover(tmp.path()).unwrap_err();

        assert!(error.to_string().contains("envie adopt"), "{error}");
    }

    #[test]
    fn resolver_uses_the_configured_naming_pattern() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(WORKSPACE_FILE), project_file("acme")).unwrap();
        let project = Project::discover(tmp.path()).unwrap();

        let resolver = project.resolver("pr-42");
        let ephemeral = resolver.resolve_environment("ephemeral").unwrap();
        let prod = resolver.resolve_environment("prod").unwrap();

        assert_eq!(ephemeral.workspace, "acme-pr-42");
        assert!(prod.is_stable());
    }

    #[test]
    fn a_project_without_units_explains_how_to_get_them() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(WORKSPACE_FILE), project_file("acme")).unwrap();
        let project = Project::discover(tmp.path()).unwrap();

        let error = project.units().unwrap_err();

        assert!(error.to_string().contains("envie adopt"), "{error}");
    }
}
