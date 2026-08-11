//! Removes the files Envie writes into a unit, and Terraform's own working state.
//!
//! Useful when switching between environments by hand, when a generated file is
//! suspected of being stale, or before committing. Nothing here is destructive to
//! infrastructure: `deploy` regenerates all of it and re-runs `terraform init`.

use crate::common::generated_files;
use crate::common::project::Project;
use crate::common::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct CleanOptions {
    pub unit_name: Option<String>,
    /// Also remove `.terraform` and the provider lock file, so the next deploy
    /// re-resolves modules and providers.
    pub deep: bool,
}

pub struct CleanCommand {
    working_directory: PathBuf,
    output_manager: OutputManager,
}

impl CleanCommand {
    pub fn new(working_directory: PathBuf) -> Self {
        Self {
            working_directory,
            output_manager: OutputManager::new(),
        }
    }

    pub fn execute(&self, options: CleanOptions) -> Result<()> {
        let project = Project::discover(&self.working_directory)?;
        let registry = project.units()?;

        // Ordered and de-duplicated: a unit reachable under several names in the
        // registry must not be cleaned, or reported, twice.
        let units: BTreeMap<String, PathBuf> = registry
            .units_by_qualified_name
            .values()
            .filter(|unit| match &options.unit_name {
                Some(name) => &unit.config.name == name || &unit.qualified_name == name,
                None => true,
            })
            .map(|unit| (unit.qualified_name.clone(), project.root.join(&unit.path)))
            .collect();

        if units.is_empty() {
            return match &options.unit_name {
                Some(name) => Err(EnvieError::ValidationError(format!(
                    "no unit called '{name}' in this project"
                ))),
                None => Err(EnvieError::ValidationError(
                    "no units found in this project".to_string(),
                )),
            };
        }

        let mut removed = 0;
        for (name, directory) in units.iter() {
            let mut cleaned = Vec::new();

            for file in generated_files::generated_file_names() {
                if directory.join(file).exists() {
                    cleaned.push(file.to_string());
                }
            }
            generated_files::clean(directory)?;

            if options.deep {
                cleaned.extend(self.remove_terraform_working_files(directory)?);
            }

            if cleaned.is_empty() {
                continue;
            }

            removed += cleaned.len();
            self.output_manager.print_blue(name);
            for file in cleaned {
                println!("  removed {file}");
            }
        }

        if removed == 0 {
            self.output_manager
                .print_green("Nothing to clean; no generated files are present.");
        } else {
            self.output_manager
                .print_green(&format!("\n✅ Removed {removed} file(s)."));
        }

        Ok(())
    }

    fn remove_terraform_working_files(&self, directory: &Path) -> Result<Vec<String>> {
        let mut removed = Vec::new();

        let terraform_dir = directory.join(".terraform");
        if terraform_dir.exists() {
            std::fs::remove_dir_all(&terraform_dir)?;
            removed.push(".terraform/".to_string());
        }

        let lock = directory.join(".terraform.lock.hcl");
        if lock.exists() {
            std::fs::remove_file(&lock)?;
            removed.push(".terraform.lock.hcl".to_string());
        }

        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn project_with_one_unit() -> TempDir {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("workspace.envie.yaml"),
            "version: \"1.0\"\nproject:\n  name: acme\n",
        )
        .unwrap();

        let unit = tmp.path().join("app");
        fs::create_dir_all(&unit).unwrap();
        fs::write(unit.join("envie.yaml"), "name: app\n").unwrap();
        fs::write(
            unit.join("main.tf"),
            "resource \"terraform_data\" \"x\" {}\n",
        )
        .unwrap();
        fs::write(unit.join("envie.generated.tf"), "locals {}\n").unwrap();
        fs::write(unit.join("envie_override.tf"), "terraform {}\n").unwrap();
        fs::create_dir_all(unit.join(".terraform")).unwrap();
        fs::write(unit.join(".terraform.lock.hcl"), "").unwrap();

        tmp
    }

    #[test]
    fn generated_files_are_removed_and_terraform_is_left_alone() {
        let tmp = project_with_one_unit();

        CleanCommand::new(tmp.path().to_path_buf())
            .execute(CleanOptions {
                unit_name: None,
                deep: false,
            })
            .unwrap();

        let unit = tmp.path().join("app");
        assert!(!unit.join("envie.generated.tf").exists());
        assert!(!unit.join("envie_override.tf").exists());
        // Terraform's own working files are expensive to rebuild, so they only go
        // when asked for.
        assert!(unit.join(".terraform").exists());
        assert!(unit.join(".terraform.lock.hcl").exists());
        // The repository's own Terraform is never touched.
        assert!(unit.join("main.tf").exists());
    }

    #[test]
    fn a_deep_clean_also_removes_terraforms_working_files() {
        let tmp = project_with_one_unit();

        CleanCommand::new(tmp.path().to_path_buf())
            .execute(CleanOptions {
                unit_name: None,
                deep: true,
            })
            .unwrap();

        let unit = tmp.path().join("app");
        assert!(!unit.join(".terraform").exists());
        assert!(!unit.join(".terraform.lock.hcl").exists());
        assert!(unit.join("main.tf").exists());
    }

    #[test]
    fn a_unit_that_does_not_exist_is_reported() {
        let tmp = project_with_one_unit();

        let error = CleanCommand::new(tmp.path().to_path_buf())
            .execute(CleanOptions {
                unit_name: Some("nope".to_string()),
                deep: false,
            })
            .unwrap_err();

        assert!(
            error.to_string().contains("no unit called 'nope'"),
            "{error}"
        );
    }
}
