//! Reads an arbitrary Terraform repository and reports what is actually in it.
//!
//! Envie's own projects are described by `envie.yaml` files, but a repository
//! that predates Envie has none. This module answers the questions Envie needs
//! in order to adopt such a repository: which directories are Terraform root
//! modules, what backend each one already uses, which cross-stack references it
//! already makes, and which inputs it expects.

use crate::common::{EnvieError, Result};
use hcl::expr::{Expression, Object, ObjectKey};
use hcl::structure::{Attribute, Block, Body};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;

/// Directory names that never contain a root module worth deploying.
const IGNORED_DIRS: &[&str] = &[
    ".git",
    ".terraform",
    ".terragrunt-cache",
    "node_modules",
    "target",
    "vendor",
    ".idea",
    ".vscode",
];

/// A `backend` block found inside a `terraform` block.
#[derive(Debug, Clone)]
pub struct BackendDecl {
    pub backend_type: String,
    /// Arguments whose values are string literals, so Envie can reuse them verbatim.
    pub config: BTreeMap<String, String>,
    /// Arguments Envie could not read, with the reason (interpolation, variable, ...).
    pub unreadable: Vec<(String, String)>,
    pub file: PathBuf,
}

impl BackendDecl {
    /// The state path this backend uses in the default workspace, if it is a literal.
    pub fn literal_key(&self) -> Option<&str> {
        match self.backend_type.as_str() {
            "s3" | "gcs" | "azurerm" | "oss" => self.config.get("key").map(String::as_str),
            "local" => self.config.get("path").map(String::as_str),
            _ => self.config.get("key").map(String::as_str),
        }
    }
}

/// Backend settings kept outside the Terraform code, for
/// `terraform init -backend-config=<file>`.
///
/// A repository that inits this way has a `backend "s3" {}` block with nothing in
/// it, so the file is the only place its bucket and key are written down. Without
/// reading it, Envie would adopt the repository with no idea where its state is.
#[derive(Debug, Clone)]
pub struct BackendConfigFile {
    /// File name, relative to the directory it was found in.
    pub file: PathBuf,
    pub config: BTreeMap<String, String>,
}

/// Whether a file looks like backend settings for `-backend-config`.
///
/// Terraform imposes no naming rule, so this follows the conventions in common
/// use rather than anything authoritative: `*.tfbackend` in any form, and `.hcl`
/// files whose name mentions the backend.
fn is_backend_config_file(name: &str) -> bool {
    if name.ends_with(".tfbackend") {
        return true;
    }

    if !name.ends_with(".hcl") || name == "versions.hcl" {
        return false;
    }

    let stem = name.trim_end_matches(".hcl").to_ascii_lowercase();
    stem == "backend" || stem.starts_with("backend") || stem.ends_with("backend")
}

/// A `data "terraform_remote_state" "<name>"` block already present in the repository.
#[derive(Debug, Clone)]
pub struct RemoteStateDecl {
    pub name: String,
    pub backend_type: Option<String>,
    pub config: BTreeMap<String, String>,
    /// The `workspace` argument, when it is a literal.
    pub workspace: Option<String>,
    pub file: PathBuf,
}

impl RemoteStateDecl {
    pub fn literal_key(&self) -> Option<&str> {
        self.config
            .get("key")
            .or_else(|| self.config.get("path"))
            .map(String::as_str)
    }
}

/// A `variable "<name>"` declaration.
#[derive(Debug, Clone)]
pub struct VariableDecl {
    pub name: String,
    pub has_default: bool,
    pub description: Option<String>,
}

/// Everything Envie learned about one directory containing `.tf` files.
#[derive(Debug, Clone, Default)]
pub struct TfDir {
    /// Path relative to the scan root.
    pub path: PathBuf,
    pub tf_files: Vec<PathBuf>,
    pub backend: Option<BackendDecl>,
    /// `module` blocks pointing at a local directory, normalised relative to the scan root.
    pub local_module_targets: Vec<PathBuf>,
    /// `module` blocks pointing at a registry, git URL, etc.
    pub remote_module_sources: Vec<String>,
    pub remote_states: Vec<RemoteStateDecl>,
    pub variables: Vec<VariableDecl>,
    pub outputs: Vec<String>,
    pub providers: Vec<String>,
    /// Names declared in `locals` blocks, so Envie can avoid redeclaring them.
    pub locals: Vec<String>,
    pub resource_count: usize,
    pub data_count: usize,
    pub has_terraform_block: bool,
    /// Whether the module reads `terraform.workspace`. Repositories that do are
    /// already naming things per environment, and Envie has to keep the workspace
    /// names they expect rather than inventing its own.
    pub uses_terraform_workspace: bool,
    /// `*.tfvars` / `*.tfvars.json` files sitting in this directory.
    pub tfvars: Vec<PathBuf>,
    /// Backend settings passed with `-backend-config`, found in this directory.
    pub backend_configs: Vec<BackendConfigFile>,
    /// Files Envie itself generated on a previous run.
    pub envie_files: Vec<PathBuf>,
    /// Files that could not be parsed, with the parser's message.
    pub parse_errors: Vec<(PathBuf, String)>,
}

impl TfDir {
    pub fn declares_variable(&self, name: &str) -> bool {
        self.variables.iter().any(|v| v.name == name)
    }

    /// Whether this directory holds the named var file.
    pub fn has_var_file(&self, file: &str) -> bool {
        self.tfvars
            .iter()
            .any(|candidate| candidate.to_string_lossy() == file)
    }

    pub fn declares_local(&self, name: &str) -> bool {
        self.locals.iter().any(|l| l == name)
    }

    pub fn remote_state(&self, name: &str) -> Option<&RemoteStateDecl> {
        self.remote_states.iter().find(|rs| rs.name == name)
    }

    /// Whether this directory creates anything, as opposed to only declaring inputs.
    pub fn manages_infrastructure(&self) -> bool {
        self.resource_count > 0
            || !self.local_module_targets.is_empty()
            || !self.remote_module_sources.is_empty()
    }

    /// Display form of the path, always with forward slashes, `.` for the root itself.
    pub fn display_path(&self) -> String {
        if self.path.as_os_str().is_empty() {
            ".".to_string()
        } else {
            self.path.to_string_lossy().replace('\\', "/")
        }
    }
}

/// Why a directory with `.tf` files is not treated as a deployable root module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotRootReason {
    /// Referenced as a local `module { source = ... }` by another directory.
    UsedAsModule { by: Vec<String> },
    /// Lives under a `modules/` directory and creates nothing on its own.
    ReusableModuleLayout,
    /// Declares no resources, no modules and no backend.
    NothingToDeploy,
    /// Under a directory conventionally excluded from deployment.
    ExcludedLocation { dir: String },
}

impl NotRootReason {
    pub fn explain(&self) -> String {
        match self {
            NotRootReason::UsedAsModule { by } => {
                format!("used as a child module by {}", by.join(", "))
            }
            NotRootReason::ReusableModuleLayout => {
                "sits under modules/ and declares no resources of its own".to_string()
            }
            NotRootReason::NothingToDeploy => {
                "declares no resources, modules or backend".to_string()
            }
            NotRootReason::ExcludedLocation { dir } => {
                format!("lives under {}/", dir)
            }
        }
    }
}

/// The result of scanning a repository.
#[derive(Debug, Clone)]
pub struct TfScan {
    pub root: PathBuf,
    /// Every directory containing `.tf` files, in stable path order.
    pub dirs: Vec<TfDir>,
    /// Indices into `dirs` that are deployable root modules.
    root_module_indices: Vec<usize>,
    /// Indices into `dirs` that are not, with the reason.
    non_root: Vec<(usize, NotRootReason)>,
}

impl TfScan {
    pub fn scan<P: AsRef<Path>>(root: P) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        if !root.is_dir() {
            return Err(EnvieError::ValidationError(format!(
                "{} is not a directory",
                root.display()
            )));
        }

        let mut dirs = scan_dirs(&root)?;
        dirs.sort_by(|a, b| a.path.cmp(&b.path));

        let (root_module_indices, non_root) = classify(&dirs);

        Ok(Self {
            root,
            dirs,
            root_module_indices,
            non_root,
        })
    }

    pub fn root_modules(&self) -> Vec<&TfDir> {
        self.root_module_indices
            .iter()
            .map(|i| &self.dirs[*i])
            .collect()
    }

    pub fn non_root_modules(&self) -> Vec<(&TfDir, &NotRootReason)> {
        self.non_root
            .iter()
            .map(|(i, r)| (&self.dirs[*i], r))
            .collect()
    }

    pub fn dir(&self, path: &Path) -> Option<&TfDir> {
        self.dirs.iter().find(|d| d.path == path)
    }

    /// Root modules that already have a backend Envie can read.
    pub fn adoptable_backends(&self) -> Vec<(&TfDir, &BackendDecl)> {
        self.root_modules()
            .into_iter()
            .filter_map(|d| d.backend.as_ref().map(|b| (d, b)))
            .collect()
    }

    /// Unparseable files, named by their path from the scan root so that the same
    /// file name in several directories can be told apart.
    pub fn parse_errors(&self) -> Vec<(PathBuf, &String)> {
        self.dirs
            .iter()
            .flat_map(|d| d.parse_errors.iter().map(|(p, e)| (d.path.join(p), e)))
            .collect()
    }
}

fn scan_dirs(root: &Path) -> Result<Vec<TfDir>> {
    let mut by_dir: BTreeMap<PathBuf, TfDir> = BTreeMap::new();

    let walker = WalkDir::new(root).into_iter().filter_entry(|e| {
        if e.depth() == 0 {
            return true;
        }
        let name = e.file_name().to_string_lossy();
        if e.file_type().is_dir() {
            !IGNORED_DIRS.contains(&name.as_ref()) && !name.starts_with('.')
        } else {
            true
        }
    });

    for entry in walker.filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        let is_tf = name.ends_with(".tf");
        let is_tfvars = name.ends_with(".tfvars") || name.ends_with(".tfvars.json");
        let is_backend_config = is_backend_config_file(&name);
        if !is_tf && !is_tfvars && !is_backend_config {
            continue;
        }

        let dir_abs = match path.parent() {
            Some(p) => p,
            None => continue,
        };
        let rel_dir = dir_abs.strip_prefix(root).unwrap_or(dir_abs).to_path_buf();

        let tf_dir = by_dir.entry(rel_dir.clone()).or_insert_with(|| TfDir {
            path: rel_dir.clone(),
            ..Default::default()
        });

        if is_tfvars {
            tf_dir.tfvars.push(PathBuf::from(&name));
            continue;
        }

        if is_backend_config {
            match std::fs::read_to_string(path).ok().and_then(|contents| {
                hcl::parse(&contents)
                    .ok()
                    .map(|body| read_literal_attributes(&body).0)
            }) {
                Some(config) if !config.is_empty() => {
                    tf_dir.backend_configs.push(BackendConfigFile {
                        file: PathBuf::from(&name),
                        config,
                    });
                }
                _ => {}
            }
            continue;
        }

        if is_envie_generated(&name) {
            tf_dir.envie_files.push(PathBuf::from(&name));
            continue;
        }

        tf_dir.tf_files.push(PathBuf::from(&name));

        let contents = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                tf_dir
                    .parse_errors
                    .push((PathBuf::from(&name), e.to_string()));
                continue;
            }
        };

        // Looked for in the raw text rather than the parsed tree: it can appear
        // anywhere an expression can, and every occurrence means the same thing.
        if contents.contains("terraform.workspace") {
            tf_dir.uses_terraform_workspace = true;
        }

        match hcl::parse(&contents) {
            Ok(body) => absorb_body(tf_dir, &rel_dir, &name, &body),
            Err(e) => tf_dir
                .parse_errors
                .push((PathBuf::from(&name), e.to_string())),
        }
    }

    hoist_settings_directories(&mut by_dir);

    // A directory may hold only tfvars; those are not Terraform directories on their own.
    Ok(by_dir
        .into_values()
        .filter(|d| !d.tf_files.is_empty() || !d.envie_files.is_empty())
        .collect())
}

/// Move per-environment settings kept in a subdirectory up to the module they
/// configure.
///
/// `config/prod.tfvars` and `config/prod.s3.tfbackend` are as common as keeping
/// those files beside `main.tf`, but a directory of nothing but settings is not a
/// Terraform module — without this they would be scanned and then dropped, and
/// the repository would look like it had neither variables nor a backend.
fn hoist_settings_directories(by_dir: &mut BTreeMap<PathBuf, TfDir>) {
    let orphaned: Vec<PathBuf> = by_dir
        .iter()
        .filter(|(_, dir)| {
            dir.tf_files.is_empty() && (!dir.backend_configs.is_empty() || !dir.tfvars.is_empty())
        })
        .map(|(path, _)| path.clone())
        .collect();

    for path in orphaned {
        let Some(owner) = path
            .ancestors()
            .skip(1)
            .find(|ancestor| {
                by_dir
                    .get(*ancestor)
                    .is_some_and(|dir| !dir.tf_files.is_empty())
            })
            .map(Path::to_path_buf)
        else {
            continue;
        };

        let Some((mut configs, tfvars)) = by_dir.get_mut(&path).map(|dir| {
            (
                std::mem::take(&mut dir.backend_configs),
                std::mem::take(&mut dir.tfvars),
            )
        }) else {
            continue;
        };

        // Keep the subdirectory in the name, so the file can be reported and
        // passed to Terraform as written.
        let prefix = path.strip_prefix(&owner).unwrap_or(&path).to_path_buf();
        for config in &mut configs {
            config.file = prefix.join(&config.file);
        }

        if let Some(dir) = by_dir.get_mut(&owner) {
            dir.backend_configs.extend(configs);
            dir.tfvars
                .extend(tfvars.into_iter().map(|file| prefix.join(file)));
        }
    }
}

/// Names of files Envie generates, which must never be read back as user intent.
pub fn is_envie_generated(file_name: &str) -> bool {
    file_name.ends_with(".envie.tf")
        || file_name == ENVIE_OVERRIDE_FILE
        || file_name == ENVIE_GENERATED_FILE
}

/// Override file, which replaces blocks the repository already declares.
pub const ENVIE_OVERRIDE_FILE: &str = "envie_override.tf";
/// Generated file, which adds blocks the repository does not declare.
pub const ENVIE_GENERATED_FILE: &str = "envie.generated.tf";

fn absorb_body(tf_dir: &mut TfDir, rel_dir: &Path, file_name: &str, body: &Body) {
    for block in body.blocks() {
        let labels: Vec<&str> = block.labels().iter().map(|l| l.as_str()).collect();

        match block.identifier() {
            "terraform" => {
                tf_dir.has_terraform_block = true;
                if let Some(backend) = find_block(block.body(), "backend") {
                    let backend_type = backend
                        .labels()
                        .first()
                        .map(|l| l.as_str().to_string())
                        .unwrap_or_default();
                    let (config, unreadable) = read_literal_attributes(backend.body());
                    tf_dir.backend = Some(BackendDecl {
                        backend_type,
                        config,
                        unreadable,
                        file: PathBuf::from(file_name),
                    });
                }
            }
            "module" => match find_attr(block.body(), "source").and_then(as_str) {
                Some(source) if is_local_source(source) => {
                    tf_dir
                        .local_module_targets
                        .push(normalize(&rel_dir.join(source)));
                }
                Some(source) => tf_dir.remote_module_sources.push(source.to_string()),
                None => {}
            },
            "data" => {
                tf_dir.data_count += 1;
                if labels.first() == Some(&"terraform_remote_state") {
                    let name = labels.get(1).copied().unwrap_or_default().to_string();
                    let backend_type = find_attr(block.body(), "backend")
                        .and_then(as_str)
                        .map(str::to_string);
                    let workspace = find_attr(block.body(), "workspace")
                        .and_then(as_str)
                        .map(str::to_string);
                    let config = find_attr(block.body(), "config")
                        .and_then(as_object)
                        .map(read_object_strings)
                        .unwrap_or_default();
                    tf_dir.remote_states.push(RemoteStateDecl {
                        name,
                        backend_type,
                        config,
                        workspace,
                        file: PathBuf::from(file_name),
                    });
                }
            }
            "resource" => tf_dir.resource_count += 1,
            "variable" => {
                if let Some(name) = labels.first() {
                    tf_dir.variables.push(VariableDecl {
                        name: name.to_string(),
                        has_default: find_attr(block.body(), "default").is_some(),
                        description: find_attr(block.body(), "description")
                            .and_then(as_str)
                            .map(str::to_string),
                    });
                }
            }
            "output" => {
                if let Some(name) = labels.first() {
                    tf_dir.outputs.push(name.to_string());
                }
            }
            "provider" => {
                if let Some(name) = labels.first() {
                    tf_dir.providers.push(name.to_string());
                }
            }
            "locals" => {
                for attribute in block.body().attributes() {
                    tf_dir.locals.push(attribute.key().to_string());
                }
            }
            _ => {}
        }
    }
}

/// Decide which scanned directories are deployable root modules.
fn classify(dirs: &[TfDir]) -> (Vec<usize>, Vec<(usize, NotRootReason)>) {
    // Directories referenced as a local module by someone else are child modules.
    let mut used_as_module: BTreeMap<PathBuf, Vec<String>> = BTreeMap::new();
    for dir in dirs {
        for target in &dir.local_module_targets {
            used_as_module
                .entry(target.clone())
                .or_default()
                .push(dir.display_path());
        }
    }

    let mut roots = Vec::new();
    let mut non_roots = Vec::new();

    for (index, dir) in dirs.iter().enumerate() {
        if let Some(by) = used_as_module.get(&dir.path) {
            let mut by = by.clone();
            by.sort();
            by.dedup();
            non_roots.push((index, NotRootReason::UsedAsModule { by }));
            continue;
        }

        if let Some(excluded) = excluded_location(&dir.path) {
            non_roots.push((index, NotRootReason::ExcludedLocation { dir: excluded }));
            continue;
        }

        // An unreferenced directory under modules/ is a library module nobody uses yet,
        // unless it manages its own infrastructure and has a backend.
        if under_modules_dir(&dir.path) && dir.backend.is_none() && dir.resource_count == 0 {
            non_roots.push((index, NotRootReason::ReusableModuleLayout));
            continue;
        }

        if !dir.manages_infrastructure() && dir.backend.is_none() {
            non_roots.push((index, NotRootReason::NothingToDeploy));
            continue;
        }

        roots.push(index);
    }

    (roots, non_roots)
}

fn excluded_location(path: &Path) -> Option<String> {
    const EXCLUDED: &[&str] = &[
        "examples", "example", "test", "tests", "fixtures", "testdata",
    ];
    path.components()
        .filter_map(|c| match c {
            Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .find(|part| EXCLUDED.contains(&part.to_lowercase().as_str()))
        .map(str::to_string)
}

fn under_modules_dir(path: &Path) -> bool {
    path.components().any(|c| match c {
        Component::Normal(s) => s.eq_ignore_ascii_case("modules"),
        _ => false,
    })
}

fn is_local_source(source: &str) -> bool {
    source.starts_with("./")
        || source.starts_with("../")
        || source == "."
        || source.starts_with('/')
}

/// Resolve `.` and `..` without touching the filesystem, so results stay relative to the scan root.
fn normalize(path: &Path) -> PathBuf {
    let mut out: Vec<std::ffi::OsString> = Vec::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            Component::Normal(s) => out.push(s.to_os_string()),
            Component::RootDir | Component::Prefix(_) => {}
        }
    }
    out.iter().collect()
}

fn find_attr<'a>(body: &'a Body, key: &str) -> Option<&'a Expression> {
    body.attributes()
        .find(|a| a.key() == key)
        .map(Attribute::expr)
}

fn find_block<'a>(body: &'a Body, identifier: &str) -> Option<&'a Block> {
    body.blocks().find(|b| b.identifier() == identifier)
}

fn as_str(expr: &Expression) -> Option<&str> {
    match expr {
        Expression::String(s) => Some(s.as_str()),
        _ => None,
    }
}

fn as_object(expr: &Expression) -> Option<&Object<ObjectKey, Expression>> {
    match expr {
        Expression::Object(obj) => Some(obj),
        _ => None,
    }
}

fn object_key_name(key: &ObjectKey) -> Option<&str> {
    match key {
        ObjectKey::Identifier(id) => Some(id.as_str()),
        ObjectKey::Expression(Expression::String(s)) => Some(s.as_str()),
        _ => None,
    }
}

fn read_object_strings(obj: &Object<ObjectKey, Expression>) -> BTreeMap<String, String> {
    obj.iter()
        .filter_map(|(k, v)| {
            let name = object_key_name(k)?;
            Some((name.to_string(), literal_to_string(v)?))
        })
        .collect()
}

/// Split a block's arguments into the ones Envie can read and the ones it cannot.
fn read_literal_attributes(body: &Body) -> (BTreeMap<String, String>, Vec<(String, String)>) {
    let mut literals = BTreeMap::new();
    let mut unreadable = Vec::new();

    for attribute in body.attributes() {
        match literal_to_string(attribute.expr()) {
            Some(value) => {
                literals.insert(attribute.key().to_string(), value);
            }
            None => unreadable.push((
                attribute.key().to_string(),
                non_literal_reason(attribute.expr()).to_string(),
            )),
        }
    }

    (literals, unreadable)
}

fn literal_to_string(expr: &Expression) -> Option<String> {
    match expr {
        Expression::String(s) => Some(s.clone()),
        Expression::Bool(b) => Some(b.to_string()),
        Expression::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn non_literal_reason(expr: &Expression) -> &'static str {
    match expr {
        Expression::TemplateExpr(_) => "uses string interpolation",
        Expression::Variable(_) => "refers to a variable",
        Expression::Traversal(_) => "reads another value",
        Expression::FuncCall(_) => "calls a function",
        Expression::Conditional(_) => "is a conditional",
        Expression::Operation(_) => "is computed",
        Expression::ForExpr(_) => "is a for expression",
        Expression::Array(_) | Expression::Object(_) => "is not a scalar",
        Expression::Null => "is null",
        _ => "is not a literal",
    }
}

/// Variable names repositories conventionally use to name resources per environment,
/// most specific first.
pub const ENV_VARIABLE_CANDIDATES: &[&str] = &[
    "environment",
    "env",
    "stage",
    "env_name",
    "environment_name",
    "name_prefix",
    "prefix",
    "namespace",
    "workspace",
    "suffix",
];

/// Pick the variable Envie should feed the environment id into, if the module declares one.
pub fn detect_env_variable(dir: &TfDir) -> Option<String> {
    let declared: BTreeSet<&str> = dir.variables.iter().map(|v| v.name.as_str()).collect();
    ENV_VARIABLE_CANDIDATES
        .iter()
        .find(|candidate| declared.contains(**candidate))
        .map(|c| c.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write(root: &Path, rel: &str, contents: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn flat_repository_is_a_single_root_module() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "main.tf",
            r#"
terraform {
  backend "s3" {
    bucket = "acme-tfstate"
    key    = "prod/terraform.tfstate"
    region = "eu-west-1"
  }
}

variable "environment" {
  default = "prod"
}

resource "aws_s3_bucket" "assets" {
  bucket = "acme-assets"
}

output "bucket" {
  value = aws_s3_bucket.assets.id
}
"#,
        );

        let scan = TfScan::scan(tmp.path()).unwrap();
        let roots = scan.root_modules();

        assert_eq!(roots.len(), 1);
        let root = roots[0];
        assert_eq!(root.display_path(), ".");
        assert_eq!(root.resource_count, 1);
        assert_eq!(root.outputs, vec!["bucket"]);

        let backend = root.backend.as_ref().expect("backend should be detected");
        assert_eq!(backend.backend_type, "s3");
        assert_eq!(backend.literal_key(), Some("prod/terraform.tfstate"));
        assert_eq!(backend.config.get("bucket").unwrap(), "acme-tfstate");
        assert_eq!(detect_env_variable(root).as_deref(), Some("environment"));
    }

    #[test]
    fn child_modules_are_not_root_modules() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "live/app/main.tf",
            r#"
module "vpc" {
  source = "../../modules/vpc"
}

resource "aws_instance" "app" {}
"#,
        );
        write(
            tmp.path(),
            "modules/vpc/main.tf",
            r#"
variable "cidr" {}
resource "aws_vpc" "this" {
  cidr_block = var.cidr
}
"#,
        );

        let scan = TfScan::scan(tmp.path()).unwrap();

        let roots: Vec<String> = scan
            .root_modules()
            .iter()
            .map(|d| d.display_path())
            .collect();
        assert_eq!(roots, vec!["live/app"]);

        let (child, reason) = scan
            .non_root_modules()
            .into_iter()
            .find(|(d, _)| d.display_path() == "modules/vpc")
            .expect("modules/vpc should be classified");
        assert_eq!(child.display_path(), "modules/vpc");
        assert_eq!(
            *reason,
            NotRootReason::UsedAsModule {
                by: vec!["live/app".to_string()]
            }
        );
    }

    #[test]
    fn existing_remote_state_dependencies_are_recorded() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "app/main.tf",
            r#"
data "terraform_remote_state" "network" {
  backend   = "s3"
  workspace = "default"
  config = {
    bucket = "acme-tfstate"
    key    = "network/terraform.tfstate"
    region = "eu-west-1"
  }
}

resource "aws_instance" "app" {
  subnet_id = data.terraform_remote_state.network.outputs.subnet_id
}
"#,
        );

        let scan = TfScan::scan(tmp.path()).unwrap();
        let app = scan.dir(Path::new("app")).unwrap();
        let dep = app.remote_state("network").unwrap();

        assert_eq!(dep.backend_type.as_deref(), Some("s3"));
        assert_eq!(dep.workspace.as_deref(), Some("default"));
        assert_eq!(dep.literal_key(), Some("network/terraform.tfstate"));
    }

    #[test]
    fn interpolated_backend_values_are_reported_not_guessed() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "main.tf",
            r#"
terraform {
  backend "s3" {
    bucket = "acme-tfstate"
    key    = "${var.environment}/terraform.tfstate"
  }
}
resource "null_resource" "x" {}
"#,
        );

        let scan = TfScan::scan(tmp.path()).unwrap();
        let backend = scan.root_modules()[0].backend.as_ref().unwrap();

        assert_eq!(backend.literal_key(), None);
        assert_eq!(
            backend.unreadable,
            vec![("key".to_string(), "uses string interpolation".to_string())]
        );
    }

    #[test]
    fn envie_generated_files_are_not_read_as_user_intent() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "main.tf",
            "resource \"null_resource\" \"x\" {}\n",
        );
        write(
            tmp.path(),
            ENVIE_OVERRIDE_FILE,
            "terraform {\n  backend \"s3\" {}\n}\n",
        );

        let scan = TfScan::scan(tmp.path()).unwrap();
        let root = scan.root_modules()[0];

        assert!(root.backend.is_none());
        assert_eq!(root.envie_files, vec![PathBuf::from(ENVIE_OVERRIDE_FILE)]);
    }

    #[test]
    fn unparseable_files_are_reported_without_failing_the_scan() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "main.tf",
            "resource \"null_resource\" \"x\" {}\n",
        );
        write(tmp.path(), "broken.tf", "resource \"oops\" {\n");

        let scan = TfScan::scan(tmp.path()).unwrap();

        assert_eq!(scan.root_modules().len(), 1);
        assert_eq!(scan.parse_errors().len(), 1);
    }
}
