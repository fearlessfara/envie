//! Turns an existing Terraform repository into an Envie project.
//!
//! Adoption is deliberately additive. It reads the repository, decides which
//! directories are deployable root modules, and writes only Envie's own
//! configuration files. The repository's Terraform code is never modified, and
//! the state it already has is recorded exactly as it is so that the first
//! deploy against the adopted environment is a no-op rather than a rebuild.

use crate::common::project::WORKSPACE_FILE;
use crate::common::tf_scan::{detect_env_variable, BackendDecl, RemoteStateDecl, TfDir, TfScan};
use crate::common::{EnvieError, OutputManager, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Marker file Envie writes into each root module.
const UNIT_FILE: &str = "envie.yaml";

#[derive(Debug, Clone)]
pub struct AdoptOptions {
    pub project_name: Option<String>,
    /// Long-lived environments to declare. The first adopts the repository's
    /// existing state; any others are declared alongside it.
    pub environments: Vec<String>,
    pub dry_run: bool,
    /// Overwrite Envie configuration that already exists.
    pub force: bool,
    pub verbose: bool,
}

/// One root module, as Envie will describe it.
#[derive(Debug, Clone)]
struct AdoptedUnit {
    name: String,
    path: PathBuf,
    resource_count: usize,
    output_count: usize,
    backend_summary: Option<String>,
    /// Existing state path, preserved so adoption does not orphan live resources.
    existing_state_key: Option<String>,
    dependencies: Vec<AdoptedDependency>,
    env_variable: Option<String>,
    /// Whether that variable already has a value in the Terraform code, which the
    /// adopted environment keeps rather than overrides.
    env_variable_has_default: bool,
    uses_terraform_workspace: bool,
    var_files: Vec<String>,
}

#[derive(Debug, Clone)]
struct AdoptedDependency {
    /// The unit being depended on.
    unit_name: String,
    /// The data source name the repository's own code already uses, when it differs.
    alias: Option<String>,
    /// How Envie worked out the edge, for the report.
    evidence: String,
}

/// The full adoption decision, computed before anything is written.
struct AdoptionPlan {
    root: PathBuf,
    project_name: String,
    /// The environment the repository's existing state becomes.
    environment_name: String,
    /// Further long-lived environments to declare alongside the adopted one.
    additional_environments: Vec<AdoptedEnvironment>,
    units: Vec<AdoptedUnit>,
    skipped: Vec<(String, String)>,
    parse_errors: Vec<(String, String)>,
    /// Backend shared by the adopted environment and new environments.
    backend: AdoptedBackend,
    existing_config: Vec<PathBuf>,
    /// How the repository already separates environments.
    strategy: Strategy,
    /// Units that are copies of one another, one per environment.
    environment_copies: Vec<String>,
}

/// A long-lived environment declared beyond the one whose state is being adopted.
///
/// It is not necessarily empty: a repository that keeps a settings file per
/// environment says where each one's state is, and Envie takes it from there.
struct AdoptedEnvironment {
    name: String,
    /// Existing state paths found for this environment, keyed by unit path.
    state_keys: BTreeMap<String, String>,
    var_files: Vec<String>,
}

/// What the repository already uses to tell its environments apart, which decides
/// how Envie has to name workspaces and state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Strategy {
    /// State paths differ per environment; the Terraform workspace is incidental.
    StatePath,
    /// `terraform.workspace` names things, and the state path is the same for
    /// every environment because the backend prefixes it by workspace. Envie must
    /// keep using workspace names the repository would recognise.
    Workspace,
}

#[derive(Debug, Clone)]
struct AdoptedBackend {
    backend_type: String,
    /// Backend arguments common to every environment, without the state path.
    config: BTreeMap<String, String>,
}

pub struct AdoptCommand {
    working_directory: PathBuf,
    output_manager: OutputManager,
}

impl AdoptCommand {
    pub fn new(working_directory: PathBuf) -> Self {
        Self {
            working_directory,
            output_manager: OutputManager::new(),
        }
    }

    pub fn execute(&self, options: AdoptOptions) -> Result<()> {
        let scan = TfScan::scan(&self.working_directory)?;
        let plan = self.plan(&scan, &options)?;

        self.report(&plan, &options);

        if !plan.existing_config.is_empty() && !options.force {
            let files: Vec<String> = plan
                .existing_config
                .iter()
                .map(|p| p.display().to_string())
                .collect();
            return Err(EnvieError::ValidationError(format!(
                "this repository already has Envie configuration ({}).\n\
                 Re-run with --force to overwrite it.",
                files.join(", ")
            )));
        }

        if options.dry_run {
            self.output_manager
                .print_yellow("\nDry run: nothing was written.");
            return Ok(());
        }

        let written = self.write(&plan)?;

        self.output_manager.print_green("\nWrote:");
        for file in &written {
            println!("  {}", file.display());
        }

        self.print_next_steps(&plan);
        Ok(())
    }

    fn plan(&self, scan: &TfScan, options: &AdoptOptions) -> Result<AdoptionPlan> {
        let scanned_roots = scan.root_modules();
        // Backends kept in `-backend-config` files are folded in first, so that
        // everything downstream sees one backend per root module regardless of
        // where the repository writes it down.
        let resolved: Vec<TfDir> = scanned_roots
            .iter()
            .map(|dir| {
                with_backend_config_file(dir, options.environments.first().map(String::as_str))
            })
            .collect();
        let root_modules: Vec<&TfDir> = resolved.iter().collect();

        if root_modules.is_empty() {
            // Unparseable files are the usual cause: a directory whose .tf files
            // could not be read looks empty, so say so rather than claiming the
            // repository has no Terraform in it.
            let mut message = format!(
                "no Terraform root modules found under {}.\n\
                 Envie looks for directories with .tf files that no other module uses as a \
                 child module. {} directories with .tf files were found in total.",
                self.working_directory.display(),
                scan.dirs.len()
            );
            let parse_errors = scan.parse_errors();
            if !parse_errors.is_empty() {
                message.push_str("\n\nSome files could not be parsed:");
                for (file, error) in parse_errors {
                    message.push_str(&format!("\n  {}: {}", file.display(), error));
                }
            }
            return Err(EnvieError::ValidationError(message));
        }

        let names = unit_names(&root_modules);
        let environment_name = options
            .environments
            .first()
            .cloned()
            .unwrap_or_else(|| infer_environment_name(&root_modules));
        let additional_environments: Vec<AdoptedEnvironment> = options
            .environments
            .iter()
            .skip(1)
            .filter(|name| *name != &environment_name)
            .map(|name| AdoptedEnvironment {
                // Read from the scan before backend settings were merged in, so
                // that this environment's file is consulted rather than the
                // adopted environment's.
                state_keys: environment_state_keys(&scanned_roots, name),
                var_files: root_modules
                    .iter()
                    .flat_map(|dir| relevant_var_files(dir, name))
                    .collect(),
                name: name.clone(),
            })
            .collect();

        let mut units = Vec::new();
        for dir in &root_modules {
            let name = names
                .get(&dir.path)
                .cloned()
                .unwrap_or_else(|| "unit".to_string());
            units.push(AdoptedUnit {
                name,
                path: dir.path.clone(),
                resource_count: dir.resource_count,
                output_count: dir.outputs.len(),
                backend_summary: dir.backend.as_ref().map(describe_backend),
                existing_state_key: dir
                    .backend
                    .as_ref()
                    .and_then(|b| b.literal_key())
                    .map(str::to_string),
                dependencies: infer_dependencies(dir, &root_modules, &names),
                env_variable: detect_env_variable(dir),
                env_variable_has_default: detect_env_variable(dir).is_some_and(|name| {
                    dir.variables
                        .iter()
                        .any(|variable| variable.name == name && variable.has_default)
                }),
                uses_terraform_workspace: dir.uses_terraform_workspace,
                var_files: relevant_var_files(dir, &environment_name),
            });
        }

        let mut existing_config = Vec::new();
        if self.working_directory.join(WORKSPACE_FILE).exists() {
            existing_config.push(PathBuf::from(WORKSPACE_FILE));
        }
        for unit in &units {
            if self
                .working_directory
                .join(&unit.path)
                .join(UNIT_FILE)
                .exists()
            {
                existing_config.push(unit.path.join(UNIT_FILE));
            }
        }

        Ok(AdoptionPlan {
            root: self.working_directory.clone(),
            project_name: options
                .project_name
                .clone()
                .unwrap_or_else(|| self.infer_project_name()),
            environment_name,
            additional_environments,
            backend: shared_backend(&root_modules),
            strategy: detect_strategy(&root_modules),
            environment_copies: environment_copies(&units),
            units,
            existing_config,
            skipped: scan
                .non_root_modules()
                .into_iter()
                .map(|(dir, reason)| (dir.display_path(), reason.explain()))
                .collect(),
            parse_errors: scan
                .parse_errors()
                .into_iter()
                .map(|(file, error)| (file.display().to_string(), error.clone()))
                .collect(),
        })
    }

    fn infer_project_name(&self) -> String {
        self.working_directory
            .file_name()
            .map(|n| slug(&n.to_string_lossy()))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "terraform-project".to_string())
    }

    fn report(&self, plan: &AdoptionPlan, options: &AdoptOptions) {
        println!("🔍 Scanned {}", plan.root.display());
        println!();

        self.output_manager
            .print_green(&format!("Root modules ({})", plan.units.len()));
        for unit in &plan.units {
            let path = display_path(&unit.path);
            println!(
                "  {:<28} {} resource(s), {} output(s)",
                path, unit.resource_count, unit.output_count
            );
            match &unit.backend_summary {
                Some(backend) => println!("  {:<28}   backend: {}", "", backend),
                None => println!("  {:<28}   no backend declared; Envie will supply one", ""),
            }
            for dependency in &unit.dependencies {
                println!(
                    "  {:<28}   depends on {} ({})",
                    "", dependency.unit_name, dependency.evidence
                );
            }
        }
        println!();

        if !plan.skipped.is_empty() {
            self.output_manager
                .print_yellow(&format!("Not deployable ({})", plan.skipped.len()));
            for (path, reason) in &plan.skipped {
                println!("  {:<28} {}", path, reason);
            }
            println!();
        }

        if !plan.parse_errors.is_empty() {
            self.output_manager
                .print_yellow(&format!("Could not parse ({})", plan.parse_errors.len()));
            for (file, error) in &plan.parse_errors {
                println!("  {:<28} {}", file, error);
            }
            println!();
        }

        self.output_manager.print_green("Environments");
        let adopted: Vec<&AdoptedUnit> = plan
            .units
            .iter()
            .filter(|u| u.existing_state_key.is_some())
            .collect();
        if adopted.is_empty() {
            println!(
                "  {:<12} no existing state found, so it starts empty",
                &plan.environment_name
            );
        } else if plan.strategy == Strategy::Workspace {
            println!(
                "  {:<12} adopts Terraform workspace '{}', where its state already is",
                &plan.environment_name, &plan.environment_name
            );
            for unit in adopted {
                println!(
                    "  {:<12}   {} → {}",
                    "",
                    unit.name,
                    unit.existing_state_key.as_deref().unwrap_or("")
                );
            }
        } else {
            println!(
                "  {:<12} adopts the state {} unit(s) already have, in workspace 'default'",
                &plan.environment_name,
                adopted.len()
            );
            for unit in adopted {
                println!(
                    "  {:<12}   {} → {}",
                    "",
                    unit.name,
                    unit.existing_state_key.as_deref().unwrap_or("")
                );
            }
        }
        for environment in &plan.additional_environments {
            match plan.strategy {
                Strategy::Workspace => println!(
                    "  {:<12} declared, using Terraform workspace '{}'",
                    environment.name, environment.name
                ),
                Strategy::StatePath if environment.state_keys.is_empty() => {
                    println!("  {:<12} declared, and starts empty", environment.name)
                }
                Strategy::StatePath => {
                    println!(
                        "  {:<12} adopts the state {} unit(s) already have",
                        environment.name,
                        environment.state_keys.len()
                    );
                    for (path, key) in &environment.state_keys {
                        let unit = plan
                            .units
                            .iter()
                            .find(|unit| &display_path(&unit.path) == path)
                            .map(|unit| unit.name.as_str())
                            .unwrap_or(path.as_str());
                        println!("  {:<12}   {} → {}", "", unit, key);
                    }
                }
            }
        }
        match plan.strategy {
            Strategy::Workspace => println!(
                "  {:<12} one per feature or pull request, each its own Terraform workspace",
                "ephemeral"
            ),
            Strategy::StatePath => println!(
                "  {:<12} one per feature or pull request, isolated by state path and workspace",
                "ephemeral"
            ),
        }
        println!();

        if plan.strategy == Strategy::Workspace {
            self.output_manager.print_yellow(
                "This repository names resources from terraform.workspace, so Envie keeps the\n\
                 workspace name equal to the environment name. Check that the workspace it\n\
                 adopts below is the one your existing state is in:",
            );
            println!(
                "  terraform workspace list   # '{}' should be there and hold your resources",
                plan.environment_name
            );
            println!();
        }

        // Two environments of the same code will fight over any resource name that
        // does not vary by environment, so say plainly how each unit will vary.
        self.output_manager.print_green("Naming per environment");
        for unit in &plan.units {
            match &unit.env_variable {
                Some(variable) if unit.env_variable_has_default => println!(
                    "  {:<28} var.{} will be set to the environment name, except in\n\
                     {:<30}'{}', which keeps the default already in the code",
                    display_path(&unit.path),
                    variable,
                    "",
                    plan.environment_name,
                ),
                Some(variable) => println!(
                    "  {:<28} var.{} will be set to the environment name",
                    display_path(&unit.path),
                    variable
                ),
                None if unit.uses_terraform_workspace => println!(
                    "  {:<28} terraform.workspace already varies per environment",
                    display_path(&unit.path)
                ),
                None => println!(
                    "  {:<28} no environment variable found — use local.envie_environment_id in\n\
                     {:<30}resource names, or names will clash between environments",
                    display_path(&unit.path),
                    ""
                ),
            }
        }
        println!();

        if !plan.environment_copies.is_empty() {
            self.output_manager.print_yellow(&format!(
                "{} are one directory per environment, so they build the same resources.\n\
                 Envie adopts them where they are and moves no state, and deploying '{}'\n\
                 leaves each of them exactly as it is today. Build a new environment from\n\
                 one of them at a time, or they will collide over shared names:",
                plan.environment_copies.join(", "),
                plan.environment_name,
            ));
            println!(
                "  envie deploy --env pr-1 --unit {}",
                plan.environment_copies
                    .first()
                    .map(String::as_str)
                    .unwrap_or("dev")
            );
            println!();
        }

        if options.verbose {
            println!(
                "Backend for every environment: {}",
                plan.backend.backend_type
            );
            for (key, value) in &plan.backend.config {
                println!("  {} = {}", key, value);
            }
            println!();
        }
    }

    fn write(&self, plan: &AdoptionPlan) -> Result<Vec<PathBuf>> {
        let mut written = Vec::new();

        let workspace_path = plan.root.join(WORKSPACE_FILE);
        std::fs::write(&workspace_path, render_workspace_config(plan))?;
        written.push(PathBuf::from(WORKSPACE_FILE));

        for unit in &plan.units {
            let path = plan.root.join(&unit.path).join(UNIT_FILE);
            std::fs::write(&path, render_unit_config(unit))?;
            written.push(unit.path.join(UNIT_FILE));
        }

        if self.update_gitignore(&plan.root)? {
            written.push(PathBuf::from(".gitignore"));
        }

        Ok(written)
    }

    /// Keep Envie's generated Terraform out of version control.
    fn update_gitignore(&self, root: &Path) -> Result<bool> {
        let path = root.join(".gitignore");
        let existing = std::fs::read_to_string(&path).unwrap_or_default();

        let entries = ["envie_override.tf", "envie.generated.tf"];
        let missing: Vec<&str> = entries
            .iter()
            .filter(|entry| !existing.lines().any(|line| line.trim() == **entry))
            .copied()
            .collect();

        if missing.is_empty() {
            return Ok(false);
        }

        let mut contents = existing;
        if !contents.is_empty() && !contents.ends_with('\n') {
            contents.push('\n');
        }
        contents.push_str("\n# Generated by Envie on every deploy\n");
        for entry in missing {
            contents.push_str(entry);
            contents.push('\n');
        }
        std::fs::write(path, contents)?;
        Ok(true)
    }

    fn print_next_steps(&self, plan: &AdoptionPlan) {
        self.output_manager.print_green("\nNext:");
        println!(
            "  envie deploy --env {} --dry-run   # check Envie found your existing state",
            plan.environment_name
        );
        println!(
            "  envie deploy --env pr-1              # build a new environment from the same code"
        );
    }
}

/// Give every root module a unique, readable name derived from its path.
fn unit_names(dirs: &[&TfDir]) -> BTreeMap<PathBuf, String> {
    let mut names: BTreeMap<PathBuf, String> = BTreeMap::new();
    let mut taken: BTreeSet<String> = BTreeSet::new();

    // Prefer the shortest suffix of the path that is unique, so `envs/dev/app`
    // and `envs/prod/app` become `dev-app` and `prod-app` rather than `app` and
    // `app-2`.
    for dir in dirs {
        let segments = name_segments(&dir.path);

        let mut chosen = None;
        for depth in 1..=segments.len().max(1) {
            let candidate = if segments.is_empty() {
                "root".to_string()
            } else {
                segments[segments.len() - depth..].join("-")
            };
            let conflicts = dirs.iter().filter(|other| {
                other.path != dir.path && suffix_name(&other.path, depth) == candidate
            });
            if conflicts.count() == 0 && !taken.contains(&candidate) {
                chosen = Some(candidate);
                break;
            }
        }

        let mut name = chosen.unwrap_or_else(|| {
            if segments.is_empty() {
                "root".to_string()
            } else {
                segments.join("-")
            }
        });
        let mut suffix = 2;
        while taken.contains(&name) {
            name = format!("{}-{}", name, suffix);
            suffix += 1;
        }

        taken.insert(name.clone());
        names.insert(dir.path.clone(), name);
    }

    names
}

fn suffix_name(path: &Path, depth: usize) -> String {
    let segments = name_segments(path);
    if segments.is_empty() {
        return "root".to_string();
    }
    let start = segments.len().saturating_sub(depth);
    segments[start..].join("-")
}

/// Units that are the same code copied once per environment.
///
/// A repository laid out as `envs/dev` and `envs/prod` has no single unit that
/// means "the application"; it has one per environment. Envie adopts them where
/// they are, which is safe, but they all build the same resource names — so
/// deploying them together into one new environment would have them collide.
/// Returning them lets adoption say so before that happens.
fn environment_copies(units: &[AdoptedUnit]) -> Vec<String> {
    const ENVIRONMENT_NAMES: [&str; 12] = [
        "dev",
        "development",
        "test",
        "qa",
        "uat",
        "stage",
        "staging",
        "preprod",
        "prod",
        "production",
        "sandbox",
        "demo",
    ];

    let mut by_parent: BTreeMap<PathBuf, Vec<&AdoptedUnit>> = BTreeMap::new();
    for unit in units {
        let leaf = unit
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if ENVIRONMENT_NAMES.contains(&leaf.as_str()) {
            by_parent
                .entry(unit.path.parent().unwrap_or(Path::new("")).to_path_buf())
                .or_default()
                .push(unit);
        }
    }

    by_parent
        .into_values()
        .filter(|siblings| siblings.len() > 1)
        .flat_map(|siblings| siblings.into_iter().map(|unit| unit.name.clone()))
        .collect()
}

/// The path segments a unit name can be built from.
///
/// A trailing directory like `terraform/` or `infra/` says nothing about which
/// unit this is — `services/api/terraform` is the api unit — so it is dropped
/// when there is a real name in front of it.
fn name_segments(path: &Path) -> Vec<String> {
    const GENERIC: [&str; 6] = [
        "terraform",
        "tf",
        "infra",
        "infrastructure",
        "iac",
        "deploy",
    ];

    let mut segments: Vec<String> = path
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => Some(slug(&s.to_string_lossy())),
            _ => None,
        })
        .filter(|s| !s.is_empty())
        .collect();

    while segments.len() > 1
        && segments
            .last()
            .is_some_and(|last| GENERIC.contains(&last.as_str()))
    {
        segments.pop();
    }

    segments
}

/// Work out which root modules read from which, using the cross-stack references
/// the repository already contains.
fn infer_dependencies(
    dir: &TfDir,
    root_modules: &[&TfDir],
    names: &BTreeMap<PathBuf, String>,
) -> Vec<AdoptedDependency> {
    let mut dependencies: Vec<AdoptedDependency> = Vec::new();

    for remote_state in &dir.remote_states {
        let candidates: Vec<&&TfDir> = root_modules
            .iter()
            .filter(|candidate| candidate.path != dir.path)
            .collect();

        let producer = producing_state_path(remote_state, &candidates)
            .or_else(|| producer_at_relative_path(remote_state, dir, &candidates))
            .or_else(|| producer_named_after(remote_state, &candidates, names));

        let Some(producer) = producer else { continue };
        let Some(unit_name) = names.get(&producer.path) else {
            continue;
        };

        let alias = (remote_state.name != *unit_name).then(|| remote_state.name.clone());
        dependencies.push(AdoptedDependency {
            unit_name: unit_name.clone(),
            alias,
            evidence: format!("data.terraform_remote_state.{}", remote_state.name),
        });
    }

    dependencies.sort_by(|a, b| a.unit_name.cmp(&b.unit_name));
    dependencies.dedup_by(|a, b| a.unit_name == b.unit_name);
    dependencies
}

/// The root module that writes the state path this remote state reads. The
/// strongest signal there is: both sides name the same location.
fn producing_state_path<'a>(
    remote_state: &RemoteStateDecl,
    candidates: &[&&'a TfDir],
) -> Option<&'a TfDir> {
    let key = remote_state.literal_key()?;
    candidates
        .iter()
        .find(|candidate| {
            candidate
                .backend
                .as_ref()
                .and_then(|backend| backend.literal_key())
                .map(|produced| produced == key)
                .unwrap_or(false)
        })
        .map(|candidate| **candidate)
}

/// The root module a relative state path points into.
///
/// A repository with no remote backend reads its neighbours directly, as in
/// `path = "../network/terraform.tfstate"`, so the directory in that path is the
/// dependency even though there is no state key to compare.
fn producer_at_relative_path<'a>(
    remote_state: &RemoteStateDecl,
    from: &TfDir,
    candidates: &[&&'a TfDir],
) -> Option<&'a TfDir> {
    let key = remote_state.literal_key()?;
    if !key.contains('/') && !key.contains('\\') {
        return None;
    }

    let referenced = normalize_relative(&from.path.join(key));
    let directory = referenced.parent()?.to_path_buf();

    candidates
        .iter()
        .find(|candidate| candidate.path == directory)
        .map(|candidate| **candidate)
}

/// The root module whose unit name matches the data source's name.
///
/// Weakest of the three, and last for that reason, but it is what makes
/// repositories work where the state path is built from variables or workspace
/// interpolation and cannot be compared literally.
fn producer_named_after<'a>(
    remote_state: &RemoteStateDecl,
    candidates: &[&&'a TfDir],
    names: &BTreeMap<PathBuf, String>,
) -> Option<&'a TfDir> {
    candidates
        .iter()
        .find(|candidate| {
            names
                .get(&candidate.path)
                .map(|name| name == &remote_state.name)
                .unwrap_or(false)
        })
        .map(|candidate| **candidate)
}

/// Resolve `.` and `..` without touching the filesystem, so a path that points
/// outside the repository simply fails to match anything.
fn normalize_relative(path: &Path) -> PathBuf {
    let mut parts: Vec<std::ffi::OsString> = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                parts.pop();
            }
            other => parts.push(other.as_os_str().to_os_string()),
        }
    }
    parts.iter().collect()
}

/// Fill in a backend that is declared empty and configured at `terraform init`.
///
/// `terraform { backend "s3" {} }` with the real settings in a `-backend-config`
/// file is a normal way to keep one root module and several environments. Envie
/// has to read that file, or it adopts the repository believing it has no state.
///
/// Where there is a file per environment, the one naming the environment being
/// adopted is the right one; a lone file is unambiguous. Anything else is left
/// alone rather than guessed at.
fn with_backend_config_file(dir: &TfDir, environment: Option<&str>) -> TfDir {
    let mut resolved = dir.clone();

    let Some(backend) = resolved.backend.as_mut() else {
        return resolved;
    };
    if backend.literal_key().is_some() || dir.backend_configs.is_empty() {
        return resolved;
    }

    let matching = environment.and_then(|environment| {
        let needle = environment.to_ascii_lowercase();
        dir.backend_configs.iter().find(|candidate| {
            candidate
                .file
                .to_string_lossy()
                .to_ascii_lowercase()
                .contains(&needle)
        })
    });

    let chosen = match matching {
        Some(chosen) => Some(chosen),
        None if dir.backend_configs.len() == 1 => dir.backend_configs.first(),
        None => None,
    };

    if let Some(chosen) = chosen {
        for (key, value) in &chosen.config {
            backend.config.entry(key.clone()).or_insert(value.clone());
        }
    }

    resolved
}

/// How the repository already separates environments.
///
/// Reading `terraform.workspace`, or prefixing state by workspace, both mean the
/// workspace name *is* the environment as far as the existing code is concerned.
/// Adopting such a repository into Terraform's `default` workspace would rename
/// every resource, which is the one thing adoption must not do.
fn detect_strategy(root_modules: &[&TfDir]) -> Strategy {
    let workspace_based = root_modules.iter().any(|dir| {
        dir.uses_terraform_workspace
            || dir
                .backend
                .as_ref()
                .map(|backend| backend.config.contains_key("workspace_key_prefix"))
                .unwrap_or(false)
    });

    if workspace_based {
        Strategy::Workspace
    } else {
        Strategy::StatePath
    }
}

/// The backend Envie will use, taken from whatever the repository already uses.
fn shared_backend(root_modules: &[&TfDir]) -> AdoptedBackend {
    let Some(backend) = root_modules
        .iter()
        .filter_map(|d| d.backend.as_ref())
        .next()
    else {
        return AdoptedBackend {
            backend_type: "local".to_string(),
            config: BTreeMap::new(),
        };
    };

    // The state path differs per unit and per environment, so it is computed
    // rather than copied. `workspace_key_prefix` is kept: it is part of where the
    // repository's existing state actually is, so dropping it would move it.
    let config = backend
        .config
        .iter()
        .filter(|(key, _)| !matches!(key.as_str(), "key" | "path"))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    AdoptedBackend {
        backend_type: backend.backend_type.clone(),
        config,
    }
}

/// Guess a name for the environment the existing state represents, from the
/// state paths themselves.
fn infer_environment_name(root_modules: &[&TfDir]) -> String {
    const KNOWN: &[&str] = &[
        "prod",
        "production",
        "staging",
        "stage",
        "preprod",
        "dev",
        "development",
        "test",
        "sandbox",
        "qa",
        "live",
    ];

    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for dir in root_modules {
        let Some(key) = dir.backend.as_ref().and_then(|b| b.literal_key()) else {
            continue;
        };
        for segment in key.split('/') {
            let segment = segment.trim_end_matches(".tfstate");
            if let Some(known) = KNOWN.iter().find(|k| k.eq_ignore_ascii_case(segment)) {
                *counts.entry(*known).or_default() += 1;
            }
        }
    }

    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(name, _)| name.to_string())
        .unwrap_or_else(|| "prod".to_string())
}

/// Where an environment other than the adopted one already keeps its state.
///
/// A repository with `config/prod.s3.tfbackend` and `config/staging.s3.tfbackend`
/// has written down the location of every environment it has, not just the one
/// being adopted. Reading the rest means those environments are adopted too,
/// rather than declared and left to start from nothing.
fn environment_state_keys(root_modules: &[&TfDir], environment: &str) -> BTreeMap<String, String> {
    let needle = environment.to_ascii_lowercase();
    let mut keys = BTreeMap::new();

    for dir in root_modules {
        // Only meaningful where the backend is chosen at init time; a backend
        // written into the Terraform is the same for every environment.
        if dir
            .backend
            .as_ref()
            .is_none_or(|b| b.literal_key().is_some())
        {
            continue;
        }

        let key = dir
            .backend_configs
            .iter()
            .find(|candidate| {
                candidate
                    .file
                    .to_string_lossy()
                    .to_ascii_lowercase()
                    .contains(&needle)
            })
            .and_then(|candidate| {
                candidate
                    .config
                    .get("key")
                    .or_else(|| candidate.config.get("path"))
            });

        if let Some(key) = key {
            keys.insert(display_path(&dir.path), key.clone());
        }
    }

    keys
}

/// Var files in this unit that look like they belong to the adopted environment.
fn relevant_var_files(dir: &TfDir, environment: &str) -> Vec<String> {
    dir.tfvars
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        // terraform.tfvars and *.auto.tfvars load by themselves.
        .filter(|name| name != "terraform.tfvars" && !name.contains(".auto."))
        .filter(|name| name.starts_with(environment) || name.contains(environment))
        .collect()
}

fn describe_backend(backend: &BackendDecl) -> String {
    let location = backend
        .config
        .get("bucket")
        .or_else(|| backend.config.get("storage_account_name"))
        .cloned();
    let key = backend.literal_key().unwrap_or("<computed>");
    match location {
        Some(location) => format!("{} → {}/{}", backend.backend_type, location, key),
        None => format!("{} → {}", backend.backend_type, key),
    }
}

fn display_path(path: &Path) -> String {
    if path.as_os_str().is_empty() {
        ".".to_string()
    } else {
        path.to_string_lossy().replace('\\', "/")
    }
}

/// Lowercase, hyphenated, safe for a Terraform workspace name and an S3 key.
fn slug(value: &str) -> String {
    let mut out = String::new();
    let mut previous_dash = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            out.push(character.to_ascii_lowercase());
            previous_dash = false;
        } else if !out.is_empty() && !previous_dash {
            out.push('-');
            previous_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

/// Render `workspace.envie.yaml`.
///
/// Written by hand rather than serialised so the file explains itself; the
/// pinned state paths in particular need a comment saying why they must not be
/// edited casually.
fn render_workspace_config(plan: &AdoptionPlan) -> String {
    let mut out = String::new();
    out.push_str("version: \"1.0\"\n\n");
    out.push_str(&format!(
        "project:\n  name: {}\n  description: Adopted from an existing Terraform repository by envie\n\n",
        plan.project_name
    ));

    out.push_str("environments:\n");

    out.push_str("  # One short-lived environment per feature, pull request or experiment.\n");
    out.push_str("  ephemeral:\n");
    match plan.strategy {
        Strategy::Workspace => {
            out.push_str(
                "    # This repository names things from terraform.workspace, so the\n\
                 \x20   # workspace is the environment id and nothing else. The state path is\n\
                 \x20   # the same for every environment; the backend's workspace_key_prefix\n\
                 \x20   # is what keeps them apart, exactly as it does today.\n",
            );
            out.push_str("    naming_pattern: \"{id}\"\n");
            out.push_str(&format!(
                "    key_pattern: \"{}\"\n",
                workspace_key_pattern(plan)
            ));
        }
        Strategy::StatePath => {
            out.push_str("    naming_pattern: \"{project}-{id}\"\n");
            out.push_str(
                "    key_pattern: \"envie/ephemeral/{id}/{unit_path}/terraform.tfstate\"\n",
            );
        }
    }
    out.push_str(&render_backend(&plan.backend, 4));
    out.push('\n');

    out.push_str("  stable:\n");
    out.push_str(&format!("    {}:\n", plan.environment_name));
    out.push_str("      description: The environment this repository already had\n");
    out.push_str(
        "      # This infrastructure was built from the variable defaults in the\n\
         \x20     # Terraform itself. Envie leaves those alone here, so deploying this\n\
         \x20     # environment changes nothing. Other environments still get the\n\
         \x20     # environment name injected, which is what keeps them apart.\n",
    );
    out.push_str("      keep_repository_defaults: true\n");
    match plan.strategy {
        Strategy::Workspace => {
            out.push_str(
                "      # The Terraform workspace this environment's state already lives in.\n",
            );
            out.push_str(&format!("      workspace: {}\n", plan.environment_name));
            out.push_str(&format!(
                "      key_pattern: \"{}\"\n",
                workspace_key_pattern(plan)
            ));
        }
        Strategy::StatePath => {
            out.push_str(
                "      # 'default' is the workspace the repository's existing state lives in.\n",
            );
            out.push_str("      workspace: default\n");
        }
    }
    out.push_str(&render_backend(&plan.backend, 6));

    let pinned: Vec<&AdoptedUnit> = plan
        .units
        .iter()
        .filter(|u| u.existing_state_key.is_some())
        .collect();
    // With workspace separation the state path is the same everywhere, so pinning
    // it per unit would say nothing that key_pattern does not already say.
    if !pinned.is_empty() && plan.strategy == Strategy::StatePath {
        out.push_str(
            "      # State paths as they already exist. Envie reuses these instead of\n\
             \x20     # deriving new ones, so deploying to this environment manages the\n\
             \x20     # infrastructure you already have rather than creating a second copy.\n",
        );
        out.push_str("      state_keys:\n");
        for unit in pinned {
            out.push_str(&format!(
                "        {}: {}\n",
                yaml_key(&display_path(&unit.path)),
                unit.existing_state_key.as_deref().unwrap_or("")
            ));
        }
    }

    let var_files: BTreeSet<String> = plan
        .units
        .iter()
        .flat_map(|u| u.var_files.iter().cloned())
        .collect();
    if !var_files.is_empty() {
        out.push_str("      # Applied per unit, and skipped where the file does not exist.\n");
        out.push_str("      var_files:\n");
        for file in var_files {
            out.push_str(&format!("        - {}\n", file));
        }
    }

    for environment in &plan.additional_environments {
        out.push('\n');
        out.push_str(&format!("    {}:\n", environment.name));
        match plan.strategy {
            Strategy::Workspace => {
                out.push_str(&format!(
                    "      # If '{}' already exists as a Terraform workspace, this manages it.\n",
                    environment.name
                ));
                out.push_str(&format!("      workspace: {}\n", environment.name));
                out.push_str(&format!(
                    "      key_pattern: \"{}\"\n",
                    workspace_key_pattern(plan)
                ));
            }
            Strategy::StatePath if environment.state_keys.is_empty() => {
                out.push_str(&format!(
                    "      # State paths are derived, so this environment starts empty. If it\n\
                     \x20     # already exists elsewhere, pin its paths with state_keys as\n\
                     \x20     # '{}' does above.\n",
                    plan.environment_name
                ));
                out.push_str(&format!("      workspace: {}\n", environment.name));
            }
            Strategy::StatePath => {
                out.push_str(
                    "      # This environment already exists too, so it keeps the values the\n\
                     \x20     # repository holds for it rather than being renamed, and stays in\n\
                     \x20     # the workspace its state is in.\n",
                );
                out.push_str("      keep_repository_defaults: true\n");
                out.push_str("      workspace: default\n");
            }
        }
        out.push_str(&render_backend(&plan.backend, 6));

        if plan.strategy == Strategy::StatePath && !environment.state_keys.is_empty() {
            out.push_str(
                "      # Found in this repository's own backend settings for this\n\
                 \x20     # environment, so deploying it manages what already exists.\n",
            );
            out.push_str("      state_keys:\n");
            for (unit, key) in &environment.state_keys {
                out.push_str(&format!("        {}: {}\n", yaml_key(unit), key));
            }
        }

        let var_files: BTreeSet<&String> = environment.var_files.iter().collect();
        if !var_files.is_empty() {
            out.push_str("      var_files:\n");
            for file in var_files {
                out.push_str(&format!("        - {}\n", file));
            }
        }
    }

    out
}

/// The state path for a workspace-separated repository.
///
/// The path itself does not vary by environment — Terraform prefixes it with the
/// workspace — so the repository's own key is reused verbatim when every unit
/// agrees on one, and qualified by unit path when they do not.
fn workspace_key_pattern(plan: &AdoptionPlan) -> String {
    let keys: BTreeSet<&str> = plan
        .units
        .iter()
        .filter_map(|unit| unit.existing_state_key.as_deref())
        .collect();

    match keys.iter().copied().next() {
        Some(key) if keys.len() == 1 => key.to_string(),
        _ => "{unit_path}/terraform.tfstate".to_string(),
    }
}

fn render_backend(backend: &AdoptedBackend, indent: usize) -> String {
    let pad = " ".repeat(indent);
    let mut out = format!("{pad}backend:\n{pad}  type: {}\n", backend.backend_type);
    if backend.config.is_empty() {
        out.push_str(&format!("{pad}  config: {{}}\n"));
    } else {
        out.push_str(&format!("{pad}  config:\n"));
        for (key, value) in &backend.config {
            out.push_str(&format!("{pad}    {}: {}\n", key, yaml_value(value)));
        }
    }
    out
}

/// Render a unit's `envie.yaml`.
fn render_unit_config(unit: &AdoptedUnit) -> String {
    let mut out = format!("name: {}\n", unit.name);
    out.push_str("unit_type: service\n");
    out.push_str("state_management: dedicated\n");

    if unit.dependencies.is_empty() {
        out.push_str("\n# No cross-stack reads were found in this module's Terraform code.\n");
        out.push_str("dependencies: []\n");
    } else {
        out.push_str(
            "\n# Inferred from the terraform_remote_state data sources already in this module.\n\
             # Envie repoints them per environment, so the same code can read from any environment.\n",
        );
        out.push_str("dependencies:\n");
        for dependency in &unit.dependencies {
            out.push_str(&format!("  - name: {}\n", dependency.unit_name));
            if let Some(alias) = &dependency.alias {
                out.push_str(&format!(
                    "    # matches data.terraform_remote_state.{} in this module\n",
                    alias
                ));
                out.push_str(&format!("    alias: {}\n", alias));
            }
        }
    }

    out
}

fn yaml_key(value: &str) -> String {
    if value == "." || value.contains(' ') {
        format!("\"{}\"", value)
    } else {
        value.to_string()
    }
}

fn yaml_value(value: &str) -> String {
    let needs_quotes = value.is_empty()
        || value.contains(": ")
        || value.contains('#')
        || value.starts_with(['*', '&', '!', '{', '[', '\'', '"', '%', '@', '`'])
        || matches!(
            value.to_ascii_lowercase().as_str(),
            "true" | "false" | "null" | "yes" | "no" | "on" | "off" | "~"
        );
    if needs_quotes {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        value.to_string()
    }
}
