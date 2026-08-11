//! Working out what Terraform should be told, for one environment.
//!
//! Deploying, destroying and deleting an environment all need the same answers:
//! which units are involved, in which order, where each one's state lives, what
//! each one reads from, and which variables it gets. Computing that once here is
//! what keeps `envie destroy` pointed at the same state `envie deploy` wrote.

use crate::common::environment::{EnvironmentResolver, ResolvedEnvironment, UnitRef};
use crate::common::generated_files::{self, RemoteStateBinding, StateLocation, UnitContext};
use crate::common::manifest;
use crate::common::project::Project;
use crate::common::tf_scan::{detect_env_variable, TfDir, TfScan};
use crate::common::unit_config::{DependencyReference, UnitType};
use crate::common::{
    resolve_units_with_prompt, DiscoveredUnit, EnvieError, Result, TerraformManager, UnitDiscovery,
    UnitRegistry,
};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct PlanRequest {
    /// Environment reference, e.g. `pr-42`, `prod` or `stable.prod`.
    pub environment: String,
    /// Unit name or path; `None` means the unit containing the current directory,
    /// or every unit.
    pub unit: Option<String>,
    /// `unit -> environment reference`, from `-E unit:environment`.
    pub environment_overrides: HashMap<String, String>,
    /// Whether to pull in the dependencies of a single requested unit.
    pub include_dependencies: bool,
    pub no_prompt: bool,
    pub verbose: bool,
}

/// Whether a unit's dependencies are read from somewhere else.
#[derive(Debug, Clone)]
pub struct PlannedDependency {
    /// Name the unit's Terraform code refers to this dependency by.
    pub data_source_name: String,
    pub unit_name: String,
    pub environment_reference: String,
    pub overridden: bool,
    pub state: StateLocation,
}

#[derive(Debug, Clone)]
pub struct PlannedUnit {
    pub name: String,
    /// Path relative to the project root.
    pub path: PathBuf,
    pub directory: PathBuf,
    pub unit_type: UnitType,
    pub target: StateLocation,
    pub dependencies: Vec<PlannedDependency>,
    pub vars: BTreeMap<String, String>,
    pub var_files: Vec<String>,
}

/// Whether a missing Terraform workspace should be created.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceMode {
    CreateIfMissing,
    RequireExisting,
}

impl PlannedUnit {
    /// Write Envie's Terraform files and initialise the directory against this
    /// environment's state.
    ///
    /// Returns `None` when the workspace does not exist and the caller asked for
    /// an existing one, which means there is nothing deployed here.
    pub fn prepare(
        &self,
        project_name: &str,
        environment: &ResolvedEnvironment,
        mode: WorkspaceMode,
        verbose: bool,
    ) -> Result<Option<TerraformManager>> {
        let context = UnitContext {
            project_name: project_name.to_string(),
            environment_id: environment.name.clone(),
            unit_name: self.name.clone(),
            workspace: environment.workspace.clone(),
        };

        let bindings: Vec<RemoteStateBinding> = self
            .dependencies
            .iter()
            .map(|dependency| RemoteStateBinding {
                data_source_name: dependency.data_source_name.clone(),
                state: dependency.state.clone(),
            })
            .collect();

        // Re-read the unit's own Terraform so generation knows what it declares.
        // Envie's previous output is ignored by the scanner, which keeps this
        // idempotent across repeated deploys.
        let scan = TfScan::scan(&self.directory)?;
        let files =
            generated_files::render(&context, &self.target, &bindings, scan.dir(Path::new("")));
        generated_files::write(&self.directory, &files)?;

        let terraform = TerraformManager::new(&self.directory).with_verbose(verbose);

        let backend_config: Vec<(String, String)> = self
            .target
            .config
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        let backend_config_refs: Vec<(&str, &str)> = backend_config
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        terraform.init_with_backend_config(&backend_config_refs)?;

        let workspace = &environment.workspace;
        if workspace == "default" {
            terraform.workspace_select(workspace)?;
            return Ok(Some(terraform));
        }

        let exists = terraform.workspace_list()?.iter().any(|w| w == workspace);
        match (exists, mode) {
            (true, _) => {
                terraform.workspace_select(workspace)?;
                Ok(Some(terraform))
            }
            (false, WorkspaceMode::CreateIfMissing) => {
                terraform.workspace_new(workspace)?;
                Ok(Some(terraform))
            }
            (false, WorkspaceMode::RequireExisting) => Ok(None),
        }
    }

    pub fn var_arguments(&self) -> Vec<(&str, &str)> {
        self.vars
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub project_name: String,
    pub environment: ResolvedEnvironment,
    /// Units in dependency order: dependencies first.
    pub units: Vec<PlannedUnit>,
    /// Things the user should know but that do not stop the run.
    pub warnings: Vec<String>,
}

impl Plan {
    /// Units in the order they should be torn down: dependents first.
    pub fn teardown_order(&self) -> Vec<&PlannedUnit> {
        self.units.iter().rev().collect()
    }

    /// Narrow the plan to units that were actually deployed.
    ///
    /// A repository usually has more units than any one environment used, and
    /// running Terraform against state that was never written is at best noise.
    pub fn retain_units(&mut self, names: &[String]) {
        self.units.retain(|unit| names.contains(&unit.name));
    }
}

pub struct Planner {
    project: Project,
    registry: UnitRegistry,
    scan: TfScan,
}

impl Planner {
    pub fn new(project: Project) -> Result<Self> {
        let registry = project.units()?;
        let scan = TfScan::scan(&project.root)?;
        Ok(Self {
            project,
            registry,
            scan,
        })
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    /// Plan a teardown, replaying how the environment was deployed.
    ///
    /// Destroying is not simply deploying in reverse: a unit that read another
    /// environment's state still needs to read it while Terraform works out what
    /// to remove, and units the environment never deployed must be left alone.
    /// Both come from what the deploy recorded. Flags given now win, so a stale
    /// or missing record can always be corrected by hand.
    pub fn plan_teardown(&self, request: &PlanRequest) -> Result<Plan> {
        let resolver = self.project.resolver(&request.environment);
        let environment = resolver.resolve_environment(&request.environment)?;
        let recorded = manifest::load(&self.project.root, &environment)?;

        let mut request = request.clone();
        if let Some(recorded) = &recorded {
            for (unit, reference) in recorded.dependency_overrides(&environment) {
                request
                    .environment_overrides
                    .entry(unit)
                    .or_insert(reference);
            }
        }

        let mut plan = self.plan(&request)?;

        match &recorded {
            Some(recorded) => {
                if request.unit.is_none() {
                    plan.retain_units(&recorded.unit_names());
                }
            }
            None => plan.warnings.push(format!(
                "Envie has no record of how '{}' was deployed. If any unit read another \
                 environment's state, pass the same -E flags you deployed with.",
                environment.name
            )),
        }

        Ok(plan)
    }

    pub fn plan(&self, request: &PlanRequest) -> Result<Plan> {
        let resolver = self.project.resolver(&request.environment);
        let environment = resolver.resolve_environment(&request.environment)?;

        let selected = self.select_units(request)?;
        let ordered = self.order_units(selected, request)?;
        let units = self.drop_units_living_elsewhere(ordered, &resolver, &environment, request)?;

        let mut planned = Vec::new();
        for unit in &units {
            planned.push(self.plan_unit(unit, &resolver, &environment, request)?);
        }

        Ok(Plan {
            project_name: self.project.name(),
            environment,
            units: planned,
            warnings: self.warnings(&units),
        })
    }

    /// Which units the request covers: an explicit selector, the unit containing
    /// the current directory, or the whole project.
    fn select_units(&self, request: &PlanRequest) -> Result<Vec<&DiscoveredUnit>> {
        if let Some(selector) = &request.unit {
            let matches = self.registry.resolve_unit(selector);
            return resolve_units_with_prompt(matches, selector, request.no_prompt);
        }

        if let Some(unit) = self.unit_containing_cwd() {
            if request.verbose {
                println!("📍 Using {} from the current directory", unit.config.name);
            }
            return Ok(vec![unit]);
        }

        Ok(self.registry.get_all_units())
    }

    fn unit_containing_cwd(&self) -> Option<&DiscoveredUnit> {
        let mut search = std::env::current_dir().ok()?;
        loop {
            if let Ok(relative) = search.strip_prefix(&self.project.root) {
                if let Some(unit) = self.registry.get_unit_by_path(&relative.to_path_buf()) {
                    return Some(unit);
                }
            }
            search = search.parent()?.to_path_buf();
        }
    }

    fn order_units<'a>(
        &'a self,
        selected: Vec<&'a DiscoveredUnit>,
        request: &PlanRequest,
    ) -> Result<Vec<&'a DiscoveredUnit>> {
        let mut discovery = UnitDiscovery::new(self.project.root.clone());
        discovery.registry = self.registry.clone();

        // A single unit is deployed together with what it reads from, so it has
        // something to read; tearing down does the opposite and touches only what
        // was asked for.
        if selected.len() == 1 && request.include_dependencies {
            let order = discovery.resolve_deployment_order(&selected[0].config.name)?;
            return Ok(self.reattach(order));
        }

        let requested: HashSet<&String> = selected.iter().map(|u| &u.qualified_name).collect();
        let ordered = discovery.get_units_in_dependency_order()?;
        Ok(self
            .reattach(ordered)
            .into_iter()
            .filter(|unit| requested.contains(&unit.qualified_name))
            .collect())
    }

    /// Map units from a throwaway registry back to this planner's registry.
    fn reattach<'a>(&'a self, units: Vec<&DiscoveredUnit>) -> Vec<&'a DiscoveredUnit> {
        units
            .into_iter()
            .filter_map(|unit| {
                self.registry
                    .get_unit_by_qualified_name(&unit.qualified_name)
            })
            .collect()
    }

    /// Drop units the user pointed at another environment.
    ///
    /// `-E network:stable.prod` means "read the production network", so acting on
    /// a copy of it inside the environment being built would be the opposite of
    /// what was asked. Naming the unit explicitly overrides that.
    fn drop_units_living_elsewhere<'a>(
        &self,
        units: Vec<&'a DiscoveredUnit>,
        resolver: &EnvironmentResolver,
        environment: &ResolvedEnvironment,
        request: &PlanRequest,
    ) -> Result<Vec<&'a DiscoveredUnit>> {
        let mut kept = Vec::new();
        for unit in units {
            let Some(reference) = request.environment_overrides.get(&unit.config.name) else {
                kept.push(unit);
                continue;
            };

            let overridden = resolver.resolve_environment(reference)?;
            let same_environment = overridden.workspace == environment.workspace
                && overridden.name == environment.name;
            let explicitly_requested = request.unit.as_deref() == Some(unit.config.name.as_str());

            if same_environment || explicitly_requested {
                kept.push(unit);
            } else if request.verbose {
                println!(
                    "⏭️  {} is read from {}, so it is left alone here",
                    unit.config.name, reference
                );
            }
        }
        Ok(kept)
    }

    fn plan_unit(
        &self,
        unit: &DiscoveredUnit,
        resolver: &EnvironmentResolver,
        environment: &ResolvedEnvironment,
        request: &PlanRequest,
    ) -> Result<PlannedUnit> {
        let unit_path = path_string(&unit.path);
        let scanned = self.scan.dir(&unit.path);

        let mut dependencies = Vec::new();
        for dependency in &unit.config.dependencies {
            let target = self.resolve_dependency(dependency, &unit.path)?;
            let target_path = path_string(&target.path);

            let override_reference = request.environment_overrides.get(&target.config.name);
            let environment_reference = override_reference
                .cloned()
                .unwrap_or_else(|| environment.reference());
            let dependency_environment = resolver.resolve_environment(&environment_reference)?;

            dependencies.push(PlannedDependency {
                data_source_name: dependency
                    .alias()
                    .cloned()
                    .unwrap_or_else(|| target.config.name.clone()),
                unit_name: target.config.name.clone(),
                environment_reference,
                overridden: override_reference.is_some(),
                state: resolver.state_location(
                    &dependency_environment,
                    UnitRef {
                        name: &target.config.name,
                        path: &target_path,
                    },
                ),
            });
        }

        Ok(PlannedUnit {
            name: unit.config.name.clone(),
            path: unit.path.clone(),
            directory: self.project.root.join(&unit.path),
            unit_type: unit.config.unit_type.clone(),
            target: resolver.state_location(
                environment,
                UnitRef {
                    name: &unit.config.name,
                    path: &unit_path,
                },
            ),
            dependencies,
            vars: unit_vars(environment, scanned),
            var_files: environment.var_files.clone(),
        })
    }

    fn resolve_dependency(
        &self,
        dependency: &DependencyReference,
        from: &Path,
    ) -> Result<&DiscoveredUnit> {
        if let Some(name) = dependency.name() {
            return self.registry.get_unit(name).ok_or_else(|| {
                EnvieError::ValidationError(format!("dependency '{}' is not a known unit", name))
            });
        }

        if let Some(path) = dependency.path() {
            let resolved = normalize_relative(&from.join(path));
            return self
                .registry
                .get_unit_by_path(&resolved)
                .or_else(|| {
                    self.registry
                        .get_unit(path.trim_start_matches("./").trim_start_matches("../"))
                })
                .ok_or_else(|| {
                    EnvieError::ValidationError(format!(
                        "dependency path '{}' (from {}) does not resolve to a unit; \
                         expected one at {}",
                        path,
                        from.display(),
                        resolved.display()
                    ))
                });
        }

        Err(EnvieError::ValidationError(
            "a dependency needs either a name or a path".to_string(),
        ))
    }

    /// Backend values Envie could not read, and files it could not parse. Both
    /// mean Envie's view of the repository is incomplete, which the user should
    /// hear about before Terraform runs.
    fn warnings(&self, units: &[&DiscoveredUnit]) -> Vec<String> {
        let mut warnings = Vec::new();

        for unit in units {
            let Some(dir) = self.scan.dir(&unit.path) else {
                warnings.push(format!(
                    "{} has no Terraform files in {}",
                    unit.config.name,
                    unit.path.display()
                ));
                continue;
            };
            if let Some(backend) = &dir.backend {
                for (key, reason) in &backend.unreadable {
                    warnings.push(format!(
                        "{}: the existing backend's '{}' {}, so Envie could not carry it over",
                        unit.config.name, key, reason
                    ));
                }
            }
        }

        for (file, error) in self.scan.parse_errors() {
            warnings.push(format!("could not parse {}: {}", file.display(), error));
        }

        warnings
    }
}

/// Variables passed to Terraform for a unit.
///
/// Beyond whatever the environment declares, Envie feeds the environment name
/// into the repository's own environment variable when it has one. Existing
/// repositories almost always name resources from something like
/// `var.environment`, and without this every environment would try to create
/// resources under the same names.
///
/// The environment a repository was adopted with is the exception. Its
/// infrastructure was built with the repository's own defaults, so overriding
/// them would rename resources — and in a repository with a directory per
/// environment, it would point one environment's code at another's name while
/// still writing that environment's state. There, a default is the answer the
/// repository already gave, and Envie leaves it alone.
fn unit_vars(
    environment: &ResolvedEnvironment,
    scanned: Option<&TfDir>,
) -> BTreeMap<String, String> {
    let mut vars = environment.vars.clone();

    let Some(dir) = scanned else { return vars };

    if let Some(name) = detect_env_variable(dir) {
        // A var file is the other way a repository writes down what an
        // environment is called, and `-var` would silently win over it.
        let repository_answers = environment
            .var_files
            .iter()
            .any(|file| dir.has_var_file(file))
            || dir
                .variables
                .iter()
                .any(|variable| variable.name == name && variable.has_default);

        if !(environment.keep_repository_defaults && repository_answers) {
            vars.entry(name).or_insert_with(|| environment.name.clone());
        }
    }

    // Terraform rejects variables a module does not declare.
    vars.retain(|name, _| dir.declares_variable(name));
    vars
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// A relative path with `.` and `..` resolved textually, for comparing a
/// declared dependency path against a discovered unit's path.
pub(crate) fn normalize_relative(path: &Path) -> PathBuf {
    let mut out: Vec<std::ffi::OsString> = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            std::path::Component::Normal(part) => out.push(part.to_os_string()),
            _ => {}
        }
    }
    out.iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// A repository laid out the way one that never used Envie would be.
    fn legacy_repository() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        fs::create_dir_all(root.join("network")).unwrap();
        fs::write(
            root.join("network/main.tf"),
            r#"
terraform {
  backend "s3" {
    bucket = "acme-tfstate"
    key    = "prod/network/terraform.tfstate"
    region = "eu-west-1"
  }
}
variable "environment" { default = "prod" }
resource "aws_vpc" "this" {}
output "vpc_id" { value = aws_vpc.this.id }
"#,
        )
        .unwrap();

        fs::create_dir_all(root.join("app")).unwrap();
        fs::write(
            root.join("app/main.tf"),
            r#"
terraform {
  backend "s3" {
    bucket = "acme-tfstate"
    key    = "prod/app/terraform.tfstate"
    region = "eu-west-1"
  }
}
variable "environment" { default = "prod" }
data "terraform_remote_state" "network" {
  backend = "s3"
  config = {
    bucket = "acme-tfstate"
    key    = "prod/network/terraform.tfstate"
  }
}
resource "aws_instance" "app" {}
"#,
        )
        .unwrap();

        tmp
    }

    fn adopt(root: &Path) {
        let command = crate::commands::adopt::AdoptCommand::new(root.to_path_buf());
        command
            .execute(crate::commands::adopt::AdoptOptions {
                project_name: Some("acme".to_string()),
                environments: Vec::new(),
                dry_run: false,
                force: false,
                verbose: false,
            })
            .unwrap();
    }

    fn planner(root: &Path) -> Planner {
        Planner::new(Project::discover(root).unwrap()).unwrap()
    }

    fn request(environment: &str) -> PlanRequest {
        PlanRequest {
            environment: environment.to_string(),
            unit: None,
            environment_overrides: HashMap::new(),
            include_dependencies: true,
            no_prompt: true,
            verbose: false,
        }
    }

    #[test]
    fn an_adopted_repository_plans_against_its_existing_state() {
        let tmp = legacy_repository();
        adopt(tmp.path());

        let plan = planner(tmp.path()).plan(&request("prod")).unwrap();

        assert!(plan.environment.is_stable());
        assert_eq!(plan.environment.workspace, "default");

        let names: Vec<&str> = plan.units.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(names, vec!["network", "app"], "dependencies come first");

        let app = plan.units.iter().find(|u| u.name == "app").unwrap();
        assert_eq!(app.target.state_path(), Some("prod/app/terraform.tfstate"));
        assert_eq!(
            app.dependencies[0].state.state_path(),
            Some("prod/network/terraform.tfstate")
        );
    }

    #[test]
    fn a_new_environment_gets_its_own_state_and_reads_within_itself() {
        let tmp = legacy_repository();
        adopt(tmp.path());

        let plan = planner(tmp.path()).plan(&request("pr-42")).unwrap();

        assert_eq!(plan.environment.workspace, "acme-pr-42");

        let app = plan.units.iter().find(|u| u.name == "app").unwrap();
        assert_eq!(
            app.target.state_path(),
            Some("envie/ephemeral/pr-42/app/terraform.tfstate")
        );
        assert_eq!(
            app.dependencies[0].state.state_path(),
            Some("envie/ephemeral/pr-42/network/terraform.tfstate")
        );
    }

    #[test]
    fn environments_never_share_a_state_path() {
        let tmp = legacy_repository();
        adopt(tmp.path());
        let planner = planner(tmp.path());

        let mut seen = HashSet::new();
        for environment in ["prod", "pr-1", "pr-2"] {
            for unit in planner.plan(&request(environment)).unwrap().units {
                let path = unit.target.state_path().unwrap().to_string();
                assert!(seen.insert(path.clone()), "{} reused {}", environment, path);
            }
        }
    }

    #[test]
    fn a_dependency_can_be_read_from_another_environment() {
        let tmp = legacy_repository();
        adopt(tmp.path());

        let mut request = request("pr-42");
        request.unit = Some("app".to_string());
        request
            .environment_overrides
            .insert("network".to_string(), "stable.prod".to_string());

        let plan = planner(tmp.path()).plan(&request).unwrap();

        let names: Vec<&str> = plan.units.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(names, vec!["app"], "the production network is not rebuilt");

        let app = &plan.units[0];
        assert_eq!(
            app.target.state_path(),
            Some("envie/ephemeral/pr-42/app/terraform.tfstate"),
            "the app itself still goes to the new environment"
        );
        assert_eq!(
            app.dependencies[0].state.state_path(),
            Some("prod/network/terraform.tfstate"),
            "but it reads production's network"
        );
        assert!(app.dependencies[0].overridden);
    }

    #[test]
    fn the_repositorys_own_environment_variable_is_wired_up() {
        let tmp = legacy_repository();
        adopt(tmp.path());

        let plan = planner(tmp.path()).plan(&request("pr-42")).unwrap();

        for unit in &plan.units {
            assert_eq!(
                unit.vars.get("environment").map(String::as_str),
                Some("pr-42"),
                "{} should have var.environment set",
                unit.name
            );
        }
    }

    #[test]
    fn variables_a_unit_does_not_declare_are_not_passed() {
        let tmp = legacy_repository();
        // A module with no variables at all must not receive any.
        fs::create_dir_all(tmp.path().join("standalone")).unwrap();
        fs::write(
            tmp.path().join("standalone/main.tf"),
            "resource \"null_resource\" \"x\" {}\n",
        )
        .unwrap();
        adopt(tmp.path());

        let plan = planner(tmp.path()).plan(&request("pr-42")).unwrap();

        let standalone = plan.units.iter().find(|u| u.name == "standalone").unwrap();
        assert!(standalone.vars.is_empty());
    }

    #[test]
    fn teardown_reverses_the_deployment_order() {
        let tmp = legacy_repository();
        adopt(tmp.path());

        let plan = planner(tmp.path()).plan(&request("pr-42")).unwrap();
        let teardown: Vec<&str> = plan
            .teardown_order()
            .iter()
            .map(|u| u.name.as_str())
            .collect();

        assert_eq!(teardown, vec!["app", "network"]);
    }

    #[test]
    fn unreadable_backend_values_become_warnings() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("app")).unwrap();
        fs::write(
            tmp.path().join("app/main.tf"),
            r#"
terraform {
  backend "s3" {
    bucket = "acme-tfstate"
    key    = "${var.environment}/app/terraform.tfstate"
  }
}
variable "environment" { default = "prod" }
resource "aws_instance" "app" {}
"#,
        )
        .unwrap();
        adopt(tmp.path());

        let plan = planner(tmp.path()).plan(&request("pr-1")).unwrap();

        assert!(
            plan.warnings.iter().any(|w| w.contains("interpolation")),
            "{:?}",
            plan.warnings
        );
    }
}
