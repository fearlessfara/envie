//! Writes the Terraform glue Envie needs into a unit directory, without editing
//! any file the repository owns.
//!
//! Two files are produced:
//!
//! * `envie_override.tf` — Terraform treats any file ending in `_override.tf` as
//!   an override, merging it over the blocks declared elsewhere in the module.
//!   Envie uses it to re-point blocks the repository *already* declares, such as
//!   an existing `data "terraform_remote_state"` that hardcodes a state path.
//! * `envie.generated.tf` — ordinary declarations for the blocks the repository
//!   does *not* declare.
//!
//! Splitting on "does the repository already declare this?" is what lets Envie
//! adopt a repository it did not create: nothing collides, because anything that
//! would collide is expressed as an override instead.

use crate::common::tf_scan::{TfDir, ENVIE_GENERATED_FILE, ENVIE_OVERRIDE_FILE};
use crate::common::Result;
use std::collections::BTreeMap;
use std::path::Path;

/// Files written by earlier versions of Envie, removed on write so that a stale
/// `backend.envie.tf` cannot collide with the current output.
const LEGACY_FILES: &[&str] = &[
    "backend.envie.tf",
    "remote_state.envie.tf",
    "locals.envie.tf",
    "variables.envie.tf",
];

/// Where one unit's Terraform state lives, for a single environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateLocation {
    pub backend_type: String,
    /// Backend arguments including the resolved state path, already using the
    /// argument name this backend expects (`key` for s3, `path` for local).
    pub config: BTreeMap<String, String>,
    /// Terraform workspace holding this state. `default` means the state lives
    /// at exactly `key`, with no workspace prefix.
    pub workspace: String,
}

impl StateLocation {
    pub fn state_path(&self) -> Option<&str> {
        self.config
            .get("key")
            .or_else(|| self.config.get("path"))
            .map(String::as_str)
    }
}

/// A dependency's state, bound to the data source name user code refers to.
#[derive(Debug, Clone)]
pub struct RemoteStateBinding {
    pub data_source_name: String,
    pub state: StateLocation,
}

/// The identity Envie exposes to a unit's Terraform code.
#[derive(Debug, Clone)]
pub struct UnitContext {
    pub project_name: String,
    pub environment_id: String,
    pub unit_name: String,
    pub workspace: String,
}

/// Rendered file contents, ready to write.
#[derive(Debug, Clone, Default)]
pub struct GeneratedFiles {
    /// `None` when the repository declares nothing that needs overriding.
    pub override_file: Option<String>,
    pub generated_file: String,
}

const HEADER: &str = "# Managed by Envie - do not edit, regenerated on every deploy.\n";

/// Render the glue files for one unit.
///
/// `scanned` describes what the unit's own Terraform code already declares. Pass
/// `None` when nothing is known, in which case every block is emitted as a plain
/// declaration.
pub fn render(
    ctx: &UnitContext,
    target: &StateLocation,
    dependencies: &[RemoteStateBinding],
    scanned: Option<&TfDir>,
) -> GeneratedFiles {
    let declares_backend = scanned.and_then(|d| d.backend.as_ref()).is_some();
    let existing_backend_type = scanned
        .and_then(|d| d.backend.as_ref())
        .map(|b| b.backend_type.as_str());

    let mut overrides = String::new();
    let mut generated = String::new();

    // A backend block already in the repository only needs replacing when its
    // type differs from the target. When the type matches, leaving the block
    // alone preserves settings Envie does not manage (profile, assume_role,
    // encrypt, ...) while `terraform init -backend-config` supplies the rest.
    match existing_backend_type {
        Some(existing) if existing != target.backend_type => {
            overrides.push_str(&render_backend_block(&target.backend_type));
        }
        Some(_) => {}
        None => {
            debug_assert!(!declares_backend);
            generated.push_str(&render_backend_block(&target.backend_type));
        }
    }

    for dependency in dependencies {
        let already_declared = scanned
            .map(|d| d.remote_state(&dependency.data_source_name).is_some())
            .unwrap_or(false);
        let block = render_remote_state_block(dependency);
        if already_declared {
            overrides.push_str(&block);
        } else {
            generated.push_str(&block);
        }
    }

    // Envie's locals are additive, but a repository may already define a local of
    // the same name, which Terraform rejects as a duplicate.
    let locals = envie_locals(ctx)
        .into_iter()
        .filter(|(name, _)| !scanned.map(|d| d.declares_local(name)).unwrap_or(false))
        .collect::<Vec<_>>();
    if !locals.is_empty() {
        generated.push_str(&render_locals_block(ctx, &locals));
    }

    GeneratedFiles {
        override_file: (!overrides.is_empty()).then(|| format!("{HEADER}\n{overrides}")),
        generated_file: format!("{HEADER}\n{generated}"),
    }
}

/// Write the rendered files into `dir`, removing anything stale first.
pub fn write(dir: &Path, files: &GeneratedFiles) -> Result<()> {
    for legacy in LEGACY_FILES {
        let path = dir.join(legacy);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
    }

    let override_path = dir.join(ENVIE_OVERRIDE_FILE);
    match &files.override_file {
        Some(contents) => std::fs::write(override_path, contents)?,
        // A leftover override from a previous deploy would keep re-pointing
        // blocks that no longer need it.
        None if override_path.exists() => std::fs::remove_file(override_path)?,
        None => {}
    }

    std::fs::write(dir.join(ENVIE_GENERATED_FILE), &files.generated_file)?;

    Ok(())
}

/// Every file Envie may have written into a unit, current and legacy.
pub fn generated_file_names() -> Vec<&'static str> {
    [ENVIE_OVERRIDE_FILE, ENVIE_GENERATED_FILE]
        .into_iter()
        .chain(LEGACY_FILES.iter().copied())
        .collect()
}

/// Remove every file Envie generates from `dir`.
pub fn clean(dir: &Path) -> Result<()> {
    for name in generated_file_names() {
        let path = dir.join(name);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

/// Backend values are supplied by `terraform init -backend-config`, so the block
/// itself stays empty. This keeps one source of truth for the state path and
/// avoids writing bucket names into files that may be committed by accident.
fn render_backend_block(backend_type: &str) -> String {
    format!("terraform {{\n  backend \"{backend_type}\" {{}}\n}}\n\n")
}

fn render_remote_state_block(dependency: &RemoteStateBinding) -> String {
    let mut block = format!(
        "data \"terraform_remote_state\" \"{}\" {{\n  backend   = \"{}\"\n  workspace = \"{}\"\n\n  config = {{\n",
        dependency.data_source_name, dependency.state.backend_type, dependency.state.workspace
    );
    let width = dependency
        .state
        .config
        .keys()
        .map(String::len)
        .max()
        .unwrap_or(0);
    for (key, value) in &dependency.state.config {
        block.push_str(&format!(
            "    {:width$} = \"{}\"\n",
            key,
            escape(value),
            width = width
        ));
    }
    block.push_str("  }\n}\n\n");
    block
}

fn envie_locals(ctx: &UnitContext) -> Vec<(&'static str, String)> {
    vec![
        ("envie_project_name", ctx.project_name.clone()),
        ("envie_environment_id", ctx.environment_id.clone()),
        ("envie_unit_name", ctx.unit_name.clone()),
        ("envie_workspace", ctx.workspace.clone()),
    ]
}

fn render_locals_block(ctx: &UnitContext, locals: &[(&str, String)]) -> String {
    let width = locals.iter().map(|(name, _)| name.len()).max().unwrap_or(0);
    let mut block = String::from("locals {\n");
    for (name, value) in locals {
        block.push_str(&format!(
            "  {:width$} = \"{}\"\n",
            name,
            escape(value),
            width = width
        ));
    }

    // Tags are only useful alongside the values they are built from.
    if locals.iter().any(|(name, _)| *name == "envie_project_name") {
        block.push_str(&format!(
            r#"
  envie_common_tags = {{
    Project     = "{}"
    Environment = "{}"
    Unit        = "{}"
    ManagedBy   = "envie"
  }}
"#,
            escape(&ctx.project_name),
            escape(&ctx.environment_id),
            escape(&ctx.unit_name),
        ));
    }

    block.push_str("}\n\n");
    block
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::tf_scan::TfScan;
    use std::fs;
    use tempfile::TempDir;

    fn ctx() -> UnitContext {
        UnitContext {
            project_name: "acme".to_string(),
            environment_id: "pr-42".to_string(),
            unit_name: "app".to_string(),
            workspace: "acme-pr-42".to_string(),
        }
    }

    fn s3_state(key: &str, workspace: &str) -> StateLocation {
        StateLocation {
            backend_type: "s3".to_string(),
            config: BTreeMap::from([
                ("bucket".to_string(), "acme-tfstate".to_string()),
                ("key".to_string(), key.to_string()),
                ("region".to_string(), "eu-west-1".to_string()),
            ]),
            workspace: workspace.to_string(),
        }
    }

    /// Scan a directory containing only the given Terraform source.
    fn scan_dir(contents: &str) -> (TempDir, TfScan) {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("main.tf"), contents).unwrap();
        let scan = TfScan::scan(tmp.path()).unwrap();
        (tmp, scan)
    }

    #[test]
    fn repository_without_a_backend_gets_one_declared() {
        let (_tmp, scan) = scan_dir("resource \"null_resource\" \"x\" {}\n");
        let dir = scan.root_modules()[0];

        let files = render(
            &ctx(),
            &s3_state("envie/app.tfstate", "acme-pr-42"),
            &[],
            Some(dir),
        );

        assert!(files.override_file.is_none());
        assert!(files.generated_file.contains("backend \"s3\" {}"));
    }

    #[test]
    fn matching_backend_type_is_left_alone_for_init_to_override() {
        let (_tmp, scan) = scan_dir(
            r#"
terraform {
  backend "s3" {
    bucket  = "legacy-bucket"
    key     = "legacy/terraform.tfstate"
    encrypt = true
  }
}
resource "null_resource" "x" {}
"#,
        );
        let dir = scan.root_modules()[0];

        let files = render(
            &ctx(),
            &s3_state("envie/app.tfstate", "acme-pr-42"),
            &[],
            Some(dir),
        );

        // Nothing is overridden, so the repository keeps `encrypt` and anything
        // else Envie does not manage; the state path comes from -backend-config.
        assert!(files.override_file.is_none());
        assert!(!files.generated_file.contains("backend"));
    }

    #[test]
    fn differing_backend_type_is_replaced_by_an_override() {
        let (_tmp, scan) = scan_dir(
            r#"
terraform {
  backend "local" {
    path = "terraform.tfstate"
  }
}
resource "null_resource" "x" {}
"#,
        );
        let dir = scan.root_modules()[0];

        let files = render(
            &ctx(),
            &s3_state("envie/app.tfstate", "acme-pr-42"),
            &[],
            Some(dir),
        );

        let override_file = files.override_file.expect("local -> s3 needs an override");
        assert!(override_file.contains("backend \"s3\" {}"));
        assert!(!files.generated_file.contains("backend"));
    }

    #[test]
    fn existing_dependency_is_repointed_rather_than_duplicated() {
        let (_tmp, scan) = scan_dir(
            r#"
data "terraform_remote_state" "network" {
  backend = "s3"
  config = {
    bucket = "acme-tfstate"
    key    = "network/terraform.tfstate"
  }
}
resource "null_resource" "x" {}
"#,
        );
        let dir = scan.root_modules()[0];

        let dependencies = vec![RemoteStateBinding {
            data_source_name: "network".to_string(),
            state: s3_state("envie/ephemeral/pr-42/network.tfstate", "acme-pr-42"),
        }];

        let files = render(
            &ctx(),
            &s3_state("envie/app.tfstate", "acme-pr-42"),
            &dependencies,
            Some(dir),
        );

        let override_file = files
            .override_file
            .expect("an already declared dependency must be overridden");
        assert!(override_file.contains("data \"terraform_remote_state\" \"network\""));
        assert!(override_file.contains("envie/ephemeral/pr-42/network.tfstate"));
        assert!(override_file.contains("workspace = \"acme-pr-42\""));
        // The generated file must not redeclare it, which Terraform would reject.
        assert!(!files.generated_file.contains("terraform_remote_state"));
    }

    #[test]
    fn new_dependency_is_declared_normally() {
        let (_tmp, scan) = scan_dir("resource \"null_resource\" \"x\" {}\n");
        let dir = scan.root_modules()[0];

        let dependencies = vec![RemoteStateBinding {
            data_source_name: "db".to_string(),
            state: s3_state("envie/db.tfstate", "default"),
        }];

        let files = render(
            &ctx(),
            &s3_state("envie/app.tfstate", "acme-pr-42"),
            &dependencies,
            Some(dir),
        );

        assert!(files.override_file.is_none());
        assert!(files
            .generated_file
            .contains("data \"terraform_remote_state\" \"db\""));
        assert!(files.generated_file.contains("workspace = \"default\""));
    }

    #[test]
    fn locals_the_repository_already_defines_are_not_redeclared() {
        let (_tmp, scan) = scan_dir(
            r#"
locals {
  envie_project_name = "hand-written"
}
resource "null_resource" "x" {}
"#,
        );
        let dir = scan.root_modules()[0];

        let files = render(
            &ctx(),
            &s3_state("envie/app.tfstate", "acme-pr-42"),
            &[],
            Some(dir),
        );

        assert!(!files.generated_file.contains("envie_project_name"));
        assert!(files.generated_file.contains("envie_environment_id"));
        // Tags reference the project name, so they are dropped with it.
        assert!(!files.generated_file.contains("envie_common_tags"));
    }

    #[test]
    fn writing_removes_files_from_older_envie_versions() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("backend.envie.tf"), "terraform {}\n").unwrap();
        fs::write(tmp.path().join("variables.envie.tf"), "variable \"x\" {}\n").unwrap();

        write(
            tmp.path(),
            &GeneratedFiles {
                override_file: None,
                generated_file: "# empty\n".to_string(),
            },
        )
        .unwrap();

        assert!(!tmp.path().join("backend.envie.tf").exists());
        assert!(!tmp.path().join("variables.envie.tf").exists());
        assert!(tmp.path().join(ENVIE_GENERATED_FILE).exists());
    }

    #[test]
    fn writing_removes_an_override_that_is_no_longer_needed() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(ENVIE_OVERRIDE_FILE), "terraform {}\n").unwrap();

        write(
            tmp.path(),
            &GeneratedFiles {
                override_file: None,
                generated_file: "# empty\n".to_string(),
            },
        )
        .unwrap();

        assert!(!tmp.path().join(ENVIE_OVERRIDE_FILE).exists());
    }

    #[test]
    fn generated_output_parses_as_terraform() {
        let (_tmp, scan) = scan_dir(
            r#"
terraform {
  backend "local" {
    path = "terraform.tfstate"
  }
}
data "terraform_remote_state" "network" {
  backend = "s3"
  config  = {}
}
resource "null_resource" "x" {}
"#,
        );
        let dir = scan.root_modules()[0];

        let dependencies = vec![
            RemoteStateBinding {
                data_source_name: "network".to_string(),
                state: s3_state("network.tfstate", "default"),
            },
            RemoteStateBinding {
                data_source_name: "db".to_string(),
                state: s3_state("db.tfstate", "acme-pr-42"),
            },
        ];

        let files = render(
            &ctx(),
            &s3_state("app.tfstate", "acme-pr-42"),
            &dependencies,
            Some(dir),
        );

        hcl::parse(files.override_file.as_ref().unwrap()).expect("override file must be valid HCL");
        hcl::parse(&files.generated_file).expect("generated file must be valid HCL");
    }
}
