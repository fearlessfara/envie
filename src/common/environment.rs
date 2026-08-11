use crate::common::generated_files::StateLocation;
use crate::common::service_config::ProjectInfo;
use crate::common::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentConfig {
    #[serde(default)]
    pub project: Option<ProjectInfo>,
    #[serde(default)]
    pub ephemeral: EphemeralConfig,
    #[serde(default)]
    pub stable: HashMap<String, StableEnvironmentConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EphemeralConfig {
    /// How the Terraform workspace name is built. Supports `{project}` and `{id}`.
    #[serde(default = "default_naming_pattern")]
    pub naming_pattern: String,
    pub backend: BackendConfig,
    /// Where state is stored. See [`StateKeyPattern`] for the placeholders.
    #[serde(default)]
    pub key_pattern: Option<String>,
    /// Terraform variables passed to every unit in an ephemeral environment.
    #[serde(default)]
    pub vars: BTreeMap<String, String>,
    /// `-var-file` arguments, relative to each unit's directory.
    #[serde(default)]
    pub var_files: Vec<String>,
}

impl Default for EphemeralConfig {
    fn default() -> Self {
        Self {
            naming_pattern: default_naming_pattern(),
            backend: BackendConfig::local(),
            key_pattern: None,
            vars: BTreeMap::new(),
            var_files: Vec::new(),
        }
    }
}

fn default_naming_pattern() -> String {
    "{project}-{id}".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StableEnvironmentConfig {
    /// Terraform workspace holding this environment's state. `default` means the
    /// state sits at exactly the configured key, with no workspace prefix, which
    /// is how a repository that never used workspaces already stores it.
    #[serde(default = "default_workspace")]
    pub workspace: String,
    pub backend: BackendConfig,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub key_pattern: Option<String>,
    /// State paths pinned to an exact value, keyed by unit path or unit name.
    ///
    /// Envie writes these when adopting a repository that already has state, so
    /// that deploying does not point Terraform at an empty state file and
    /// propose recreating live infrastructure.
    #[serde(default)]
    pub state_keys: BTreeMap<String, String>,
    /// Leave variables that already have a default in the Terraform code alone.
    ///
    /// Set on the environment a repository was adopted with. Its infrastructure
    /// was built from those defaults, so replacing them with the environment name
    /// would rename resources — and where each environment is its own directory,
    /// it would apply one environment's name to another environment's state.
    #[serde(default)]
    pub keep_repository_defaults: bool,
    #[serde(default)]
    pub vars: BTreeMap<String, String>,
    #[serde(default)]
    pub var_files: Vec<String>,
}

fn default_workspace() -> String {
    "default".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendConfig {
    #[serde(rename = "type")]
    pub backend_type: String,
    #[serde(default)]
    pub config: BTreeMap<String, String>,
}

impl BackendConfig {
    pub fn local() -> Self {
        Self {
            backend_type: "local".to_string(),
            config: BTreeMap::new(),
        }
    }

    /// The argument this backend uses for the state path.
    pub fn state_path_argument(&self) -> &'static str {
        match self.backend_type.as_str() {
            "local" => "path",
            _ => "key",
        }
    }

    /// Backend arguments to pass to `terraform init`, excluding Envie's own
    /// bookkeeping and the state path, which is computed per unit.
    pub fn init_arguments(&self) -> BTreeMap<String, String> {
        self.config
            .iter()
            .filter(|(key, _)| !matches!(key.as_str(), "key_pattern" | "key" | "path"))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvironmentType {
    Ephemeral,
    Stable(String),
}

#[derive(Debug, Clone)]
pub struct ResolvedEnvironment {
    /// Human-facing identifier: the stable environment's name, or the ephemeral id.
    pub name: String,
    pub workspace: String,
    pub environment_type: EnvironmentType,
    pub backend: BackendConfig,
    pub key_pattern: Option<String>,
    pub pinned_state_keys: BTreeMap<String, String>,
    /// See `StableEnvironmentConfig::keep_repository_defaults`.
    pub keep_repository_defaults: bool,
    pub vars: BTreeMap<String, String>,
    pub var_files: Vec<String>,
}

impl ResolvedEnvironment {
    pub fn is_stable(&self) -> bool {
        matches!(self.environment_type, EnvironmentType::Stable(_))
    }

    /// A reference that resolves back to this exact environment.
    pub fn reference(&self) -> String {
        match &self.environment_type {
            EnvironmentType::Stable(name) => format!("stable.{}", name),
            EnvironmentType::Ephemeral => format!("ephemeral.{}", self.name),
        }
    }

    /// An ephemeral environment with nothing configured beyond a backend type,
    /// for tests that need an environment but not a project.
    #[cfg(test)]
    pub fn for_tests(name: &str, workspace: &str, backend_type: &str) -> Self {
        Self {
            name: name.to_string(),
            workspace: workspace.to_string(),
            environment_type: EnvironmentType::Ephemeral,
            backend: BackendConfig {
                backend_type: backend_type.to_string(),
                config: BTreeMap::new(),
            },
            key_pattern: None,
            pinned_state_keys: BTreeMap::new(),
            keep_repository_defaults: false,
            vars: BTreeMap::new(),
            var_files: Vec::new(),
        }
    }
}

/// Identifies the unit a state path is being computed for.
#[derive(Debug, Clone, Copy)]
pub struct UnitRef<'a> {
    pub name: &'a str,
    /// Path relative to the project root, e.g. `live/app`.
    pub path: &'a str,
}

const DEFAULT_EPHEMERAL_KEY_PATTERN: &str = "envie/ephemeral/{id}/{unit_path}/terraform.tfstate";
const DEFAULT_STABLE_KEY_PATTERN: &str = "envie/{environment}/{unit_path}/terraform.tfstate";

/// A state path pinned for this unit, if the environment has one.
///
/// A unit is referred to by several equivalent spellings — a repository root is
/// `""`, `"."` or the unit's name depending on who is asking — and a pin that
/// silently fails to match would send Envie at a fresh state file and rebuild
/// infrastructure that already exists. So every spelling is tried.
fn pinned_state_key(env: &ResolvedEnvironment, unit: UnitRef<'_>) -> Option<String> {
    if env.pinned_state_keys.is_empty() {
        return None;
    }

    let trimmed = unit
        .path
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_matches('/')
        .to_string();

    let mut candidates = vec![unit.path.to_string(), trimmed.clone()];
    if trimmed.is_empty() || trimmed == "." {
        candidates.push(".".to_string());
        candidates.push(String::new());
    }
    candidates.push(unit.name.to_string());

    candidates
        .iter()
        .find_map(|candidate| env.pinned_state_keys.get(candidate))
        .cloned()
}

#[derive(Debug, Clone)]
pub struct EnvironmentResolver {
    /// Workspace of the deployment currently in progress, used by the bare
    /// `ephemeral` reference.
    pub current_workspace: String,
    /// Environment id of the deployment currently in progress.
    pub current_environment_id: String,
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
            current_environment_id: String::new(),
            project_name,
            available_workspaces: Vec::new(),
            environment_config,
        }
    }

    pub fn with_available_workspaces(mut self, workspaces: Vec<String>) -> Self {
        self.available_workspaces = workspaces;
        self
    }

    pub fn with_current_environment_id(mut self, environment_id: String) -> Self {
        self.current_environment_id = environment_id;
        self
    }

    /// Names of the declared stable environments, sorted.
    pub fn stable_environment_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.environment_config.stable.keys().cloned().collect();
        names.sort();
        names
    }

    /// Resolve an environment reference.
    ///
    /// Accepted forms:
    /// * `stable.prod` — the declared stable environment `prod`
    /// * `ephemeral` — the deployment currently in progress
    /// * `ephemeral.pr-42` — a specific ephemeral environment
    /// * `prod` — `stable.prod` when declared, otherwise the ephemeral id `prod`
    pub fn resolve_environment(&self, env_ref: &str) -> Result<ResolvedEnvironment> {
        if let Some(name) = env_ref.strip_prefix("stable.") {
            self.resolve_stable_environment(name)
        } else if env_ref == "ephemeral" {
            let id = if self.current_environment_id.is_empty() {
                self.current_workspace.clone()
            } else {
                self.current_environment_id.clone()
            };
            self.resolve_ephemeral(&id, Some(self.current_workspace.clone()))
        } else if let Some(id) = env_ref.strip_prefix("ephemeral.") {
            self.resolve_ephemeral(id, None)
        } else if self.environment_config.stable.contains_key(env_ref) {
            // A bare name matching a declared stable environment means that
            // environment, so `--env prod` deploys to prod rather than creating
            // an ephemeral environment that happens to be called "prod".
            self.resolve_stable_environment(env_ref)
        } else {
            self.resolve_ephemeral(env_ref, None)
        }
    }

    fn resolve_stable_environment(&self, name: &str) -> Result<ResolvedEnvironment> {
        let stable = self.environment_config.stable.get(name).ok_or_else(|| {
            let available = self.stable_environment_names();
            let hint = if available.is_empty() {
                "no stable environments are declared in workspace.envie.yaml".to_string()
            } else {
                format!("declared stable environments: {}", available.join(", "))
            };
            EnvieError::ValidationError(format!("unknown stable environment '{}' ({})", name, hint))
        })?;

        Ok(ResolvedEnvironment {
            name: name.to_string(),
            workspace: stable.workspace.clone(),
            environment_type: EnvironmentType::Stable(name.to_string()),
            backend: stable.backend.clone(),
            key_pattern: stable.key_pattern.clone(),
            pinned_state_keys: stable.state_keys.clone(),
            keep_repository_defaults: stable.keep_repository_defaults,
            vars: stable.vars.clone(),
            var_files: stable.var_files.clone(),
        })
    }

    fn resolve_ephemeral(
        &self,
        id: &str,
        workspace: Option<String>,
    ) -> Result<ResolvedEnvironment> {
        if id.is_empty() {
            return Err(EnvieError::ValidationError(
                "an ephemeral environment needs an id".to_string(),
            ));
        }

        let ephemeral = &self.environment_config.ephemeral;
        let workspace = workspace.unwrap_or_else(|| self.ephemeral_workspace(id));

        // Only validate against known workspaces when the caller supplied a list;
        // an empty list means "not looked up", not "none exist".
        if !self.available_workspaces.is_empty() && !self.available_workspaces.contains(&workspace)
        {
            return Err(EnvieError::ValidationError(format!(
                "ephemeral environment '{}' (workspace '{}') does not exist. Existing: {}",
                id,
                workspace,
                self.available_workspaces.join(", ")
            )));
        }

        Ok(ResolvedEnvironment {
            name: id.to_string(),
            workspace,
            environment_type: EnvironmentType::Ephemeral,
            backend: ephemeral.backend.clone(),
            key_pattern: ephemeral.key_pattern.clone(),
            pinned_state_keys: BTreeMap::new(),
            // A new environment has no existing resources to preserve the names of.
            keep_repository_defaults: false,
            vars: ephemeral.vars.clone(),
            var_files: ephemeral.var_files.clone(),
        })
    }

    /// Build an ephemeral workspace name from the configured naming pattern.
    pub fn ephemeral_workspace(&self, id: &str) -> String {
        self.environment_config
            .ephemeral
            .naming_pattern
            .replace("{project}", &self.project_name)
            .replace("{id}", id)
            .replace("{env_id}", id)
    }

    /// Where a unit's state lives in a given environment.
    pub fn state_location(&self, env: &ResolvedEnvironment, unit: UnitRef<'_>) -> StateLocation {
        let mut config = env.backend.init_arguments();
        config.insert(
            env.backend.state_path_argument().to_string(),
            self.state_key(env, unit),
        );

        StateLocation {
            backend_type: env.backend.backend_type.clone(),
            config,
            workspace: env.workspace.clone(),
        }
    }

    /// The state path for a unit, preferring a pinned value so that adopted
    /// environments keep pointing at the state they already have.
    pub fn state_key(&self, env: &ResolvedEnvironment, unit: UnitRef<'_>) -> String {
        if let Some(pinned) = pinned_state_key(env, unit) {
            return pinned;
        }

        let pattern = env
            .key_pattern
            .clone()
            .or_else(|| env.backend.config.get("key_pattern").cloned())
            .unwrap_or_else(|| match env.environment_type {
                EnvironmentType::Ephemeral => DEFAULT_EPHEMERAL_KEY_PATTERN.to_string(),
                EnvironmentType::Stable(_) => DEFAULT_STABLE_KEY_PATTERN.to_string(),
            });

        let unit_path = if unit.path.is_empty() || unit.path == "." {
            unit.name.to_string()
        } else {
            unit.path.replace('\\', "/").trim_matches('/').to_string()
        };

        pattern
            .replace("{project}", &self.project_name)
            .replace("{environment}", &env.name)
            .replace("{env_id}", &env.name)
            .replace("{id}", &env.name)
            .replace("{workspace}", &env.workspace)
            .replace("{unit_path}", &unit_path)
            .replace("{path}", &unit_path)
            .replace("{unit}", unit.name)
            // Retained so key patterns written for the earlier service/module
            // layout keep resolving.
            .replace("{service}", unit.name)
            .replace("{module}", unit.name)
    }
}

impl EnvironmentConfig {
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_str(&content)
    }

    pub fn from_str(content: &str) -> Result<Self> {
        serde_yaml::from_str(content).map_err(|e| {
            EnvieError::ConfigError(format!("Failed to parse environment config: {}", e))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s3_backend(bucket: &str) -> BackendConfig {
        BackendConfig {
            backend_type: "s3".to_string(),
            config: BTreeMap::from([
                ("bucket".to_string(), bucket.to_string()),
                ("region".to_string(), "eu-west-1".to_string()),
            ]),
        }
    }

    fn resolver() -> EnvironmentResolver {
        let mut stable = HashMap::new();
        stable.insert(
            "prod".to_string(),
            StableEnvironmentConfig {
                workspace: "default".to_string(),
                backend: s3_backend("acme-tfstate"),
                description: "Adopted".to_string(),
                key_pattern: None,
                state_keys: BTreeMap::from([(
                    "live/app".to_string(),
                    "legacy/app/terraform.tfstate".to_string(),
                )]),
                keep_repository_defaults: false,
                vars: BTreeMap::from([("environment".to_string(), "prod".to_string())]),
                var_files: vec!["prod.tfvars".to_string()],
            },
        );

        let config = EnvironmentConfig {
            project: None,
            ephemeral: EphemeralConfig {
                naming_pattern: "{project}-{id}".to_string(),
                backend: s3_backend("acme-tfstate"),
                key_pattern: None,
                vars: BTreeMap::new(),
                var_files: Vec::new(),
            },
            stable,
        };

        EnvironmentResolver::new("acme-pr-42".to_string(), "acme".to_string(), config)
            .with_current_environment_id("pr-42".to_string())
    }

    #[test]
    fn bare_name_of_a_declared_environment_resolves_to_it() {
        let resolved = resolver().resolve_environment("prod").unwrap();

        assert!(resolved.is_stable());
        assert_eq!(resolved.workspace, "default");
        assert_eq!(resolved.name, "prod");
    }

    #[test]
    fn bare_name_that_is_not_declared_is_an_ephemeral_id() {
        let resolved = resolver().resolve_environment("pr-99").unwrap();

        assert_eq!(resolved.environment_type, EnvironmentType::Ephemeral);
        assert_eq!(resolved.workspace, "acme-pr-99");
        assert_eq!(resolved.name, "pr-99");
    }

    #[test]
    fn bare_ephemeral_means_the_deployment_in_progress() {
        let resolved = resolver().resolve_environment("ephemeral").unwrap();

        assert_eq!(resolved.workspace, "acme-pr-42");
        assert_eq!(resolved.name, "pr-42");
    }

    #[test]
    fn a_specific_ephemeral_environment_can_be_referenced() {
        let resolved = resolver().resolve_environment("ephemeral.pr-7").unwrap();

        assert_eq!(resolved.workspace, "acme-pr-7");
    }

    #[test]
    fn unknown_stable_environment_lists_what_is_declared() {
        let error = resolver()
            .resolve_environment("stable.staging")
            .unwrap_err();

        let message = error.to_string();
        assert!(message.contains("staging"), "{message}");
        assert!(message.contains("prod"), "{message}");
    }

    #[test]
    fn pinned_state_keys_win_so_adopted_state_is_reused() {
        let resolver = resolver();
        let prod = resolver.resolve_environment("prod").unwrap();

        let key = resolver.state_key(
            &prod,
            UnitRef {
                name: "app",
                path: "live/app",
            },
        );

        assert_eq!(key, "legacy/app/terraform.tfstate");
    }

    #[test]
    fn units_without_a_pinned_key_follow_the_pattern() {
        let resolver = resolver();
        let prod = resolver.resolve_environment("prod").unwrap();

        let key = resolver.state_key(
            &prod,
            UnitRef {
                name: "db",
                path: "live/db",
            },
        );

        assert_eq!(key, "envie/prod/live/db/terraform.tfstate");
    }

    #[test]
    fn ephemeral_state_keys_are_isolated_per_environment() {
        let resolver = resolver();

        let first = resolver.state_key(
            &resolver.resolve_environment("pr-1").unwrap(),
            UnitRef {
                name: "app",
                path: "live/app",
            },
        );
        let second = resolver.state_key(
            &resolver.resolve_environment("pr-2").unwrap(),
            UnitRef {
                name: "app",
                path: "live/app",
            },
        );

        assert_eq!(first, "envie/ephemeral/pr-1/live/app/terraform.tfstate");
        assert_ne!(first, second);
    }

    #[test]
    fn state_location_uses_the_argument_the_backend_expects() {
        let resolver = resolver();
        let env = resolver.resolve_environment("pr-1").unwrap();

        let location = resolver.state_location(
            &env,
            UnitRef {
                name: "app",
                path: "live/app",
            },
        );

        assert_eq!(location.backend_type, "s3");
        assert_eq!(location.config.get("bucket").unwrap(), "acme-tfstate");
        assert_eq!(
            location.state_path(),
            Some("envie/ephemeral/pr-1/live/app/terraform.tfstate")
        );

        let local = BackendConfig::local();
        assert_eq!(local.state_path_argument(), "path");
    }

    #[test]
    fn legacy_service_module_key_patterns_still_resolve() {
        let mut resolver = resolver();
        resolver.environment_config.ephemeral.backend.config.insert(
            "key_pattern".to_string(),
            "{service}/{module}/terraform.tfstate".to_string(),
        );

        let env = resolver.resolve_environment("pr-1").unwrap();
        let key = resolver.state_key(
            &env,
            UnitRef {
                name: "api",
                path: "units/api",
            },
        );

        assert_eq!(key, "api/api/terraform.tfstate");
    }

    #[test]
    fn key_pattern_is_kept_out_of_the_backend_arguments() {
        let mut backend = s3_backend("acme-tfstate");
        backend
            .config
            .insert("key_pattern".to_string(), "x/{unit}".to_string());
        backend
            .config
            .insert("key".to_string(), "stale".to_string());

        let arguments = backend.init_arguments();

        assert!(!arguments.contains_key("key_pattern"));
        assert!(!arguments.contains_key("key"));
        assert_eq!(arguments.get("bucket").unwrap(), "acme-tfstate");
    }

    #[test]
    fn environment_vars_and_var_files_are_carried_through() {
        let prod = resolver().resolve_environment("prod").unwrap();

        assert_eq!(prod.vars.get("environment").unwrap(), "prod");
        assert_eq!(prod.var_files, vec!["prod.tfvars"]);
    }
}
