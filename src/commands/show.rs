//! The units in a project, and what depends on what.
//!
//! The counterpart to `envie list`: that answers which environments exist, this
//! answers what an environment is made of. Neither runs Terraform.

use crate::common::deployment::normalize_relative;
use crate::common::project::Project;
use crate::common::unit_config::{DependencyReference, StateManagement, UnitType};
use crate::common::*;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ShowOptions {
    pub unit: Option<String>,
    pub verbose: bool,
}

pub struct ShowCommand {
    working_directory: PathBuf,
    output_manager: OutputManager,
}

impl ShowCommand {
    pub fn new(working_directory: PathBuf) -> Self {
        Self {
            working_directory,
            output_manager: OutputManager::new(),
        }
    }

    pub fn execute(&self, options: ShowOptions) -> Result<()> {
        // Discovery starts at the project root, not the current directory, so
        // that running this from inside a unit still describes the whole project.
        let project = Project::discover(&self.working_directory)?;
        let registry = project.units()?;

        if options.verbose {
            println!("🔍 Reading {}\n", project.root.display());
        }

        let mut units = registry.get_all_units();
        units.sort_by(|a, b| a.path.cmp(&b.path));

        match &options.unit {
            Some(name) => {
                let unit = registry.get_unit(name).ok_or_else(|| {
                    let known: Vec<&str> =
                        units.iter().map(|unit| unit.config.name.as_str()).collect();
                    EnvieError::ValidationError(format!(
                        "no unit called '{}' (units in this project: {})",
                        name,
                        known.join(", ")
                    ))
                })?;
                self.print_unit(unit, &units);
            }
            None => self.print_all(&project, &units),
        }

        Ok(())
    }

    fn print_all(&self, project: &Project, units: &[&DiscoveredUnit]) {
        self.output_manager
            .print_green(&format!("📋 Units in {}\n", project.name()));

        let width = units
            .iter()
            .map(|unit| unit.config.name.len())
            .max()
            .unwrap_or(0)
            .max(4);

        for unit in units {
            println!(
                "  {:width$}  {}",
                unit.config.name,
                display_path(&unit.path),
                width = width
            );
            if !unit.config.description.is_empty() {
                println!(
                    "  {:width$}  {}",
                    "",
                    unit.config.description,
                    width = width
                );
            }
            if !unit.config.dependencies.is_empty() {
                println!(
                    "  {:width$}  reads {}",
                    "",
                    unit.config
                        .dependencies
                        .iter()
                        .map(dependency_label)
                        .collect::<Vec<_>>()
                        .join(", "),
                    width = width
                );
            }
        }

        println!("\nEnvironments: envie list");
    }

    fn print_unit(&self, unit: &DiscoveredUnit, all: &[&DiscoveredUnit]) {
        self.output_manager
            .print_green(&format!("📦 {}\n", unit.config.name));

        if !unit.config.description.is_empty() {
            println!("  {}\n", unit.config.description);
        }

        println!("  path     {}", display_path(&unit.path));
        println!("  type     {}", describe_type(&unit.config.unit_type));
        println!(
            "  state    {}",
            describe_state(&unit.config.state_management)
        );

        let reads: Vec<String> = unit
            .config
            .dependencies
            .iter()
            .map(|dependency| match resolve(dependency, &unit.path, all) {
                Some(target) => format!("{} ({})", target.config.name, display_path(&target.path)),
                // Worth saying out loud: a dependency that resolves to nothing is
                // why a deploy will fail, and it is invisible otherwise.
                None => format!("{} — no such unit", dependency_label(dependency)),
            })
            .collect();
        println!(
            "  reads    {}",
            if reads.is_empty() {
                "nothing".to_string()
            } else {
                reads.join(", ")
            }
        );

        let read_by: Vec<&str> = all
            .iter()
            .filter(|other| {
                other.config.dependencies.iter().any(|dependency| {
                    resolve(dependency, &other.path, all)
                        .is_some_and(|target| target.path == unit.path)
                })
            })
            .map(|other| other.config.name.as_str())
            .collect();
        println!(
            "  read by  {}",
            if read_by.is_empty() {
                "nothing".to_string()
            } else {
                read_by.join(", ")
            }
        );
    }
}

fn display_path(path: &Path) -> String {
    let shown = path.to_string_lossy().replace('\\', "/");
    if shown.is_empty() {
        ".".to_string()
    } else {
        shown
    }
}

fn dependency_label(dependency: &DependencyReference) -> String {
    dependency
        .name()
        .or_else(|| dependency.path())
        .cloned()
        .unwrap_or_else(|| "unnamed".to_string())
}

/// The unit a dependency refers to, by name or by path relative to `from`.
fn resolve<'a>(
    dependency: &DependencyReference,
    from: &Path,
    all: &[&'a DiscoveredUnit],
) -> Option<&'a DiscoveredUnit> {
    if let Some(name) = dependency.name() {
        return all.iter().find(|unit| &unit.config.name == name).copied();
    }

    let path = dependency.path()?;
    let resolved = normalize_relative(&from.join(path));
    all.iter()
        .find(|unit| unit.path == resolved)
        .or_else(|| {
            let trimmed = path.trim_start_matches("./").trim_start_matches("../");
            all.iter().find(|unit| unit.config.name == trimmed)
        })
        .copied()
}

fn describe_type(unit_type: &UnitType) -> String {
    match unit_type {
        UnitType::Service => "service".to_string(),
        UnitType::Module => "module".to_string(),
        UnitType::Component => "component".to_string(),
        UnitType::Layer => "layer".to_string(),
        UnitType::Application => "application".to_string(),
        UnitType::Custom(name) => name.clone(),
    }
}

fn describe_state(state: &StateManagement) -> String {
    match state {
        StateManagement::Dedicated => "its own state file".to_string(),
        StateManagement::Parent => "shares its parent unit's state".to_string(),
        StateManagement::Shared(id) => format!("shares the state '{}'", id),
        StateManagement::Group(id) => format!("part of the state group '{}'", id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flat_root_shows_as_a_dot_rather_than_nothing() {
        assert_eq!(display_path(Path::new("")), ".");
        assert_eq!(display_path(Path::new("units/api")), "units/api");
    }

    #[test]
    fn descriptions_avoid_leaking_rust_names() {
        assert_eq!(describe_type(&UnitType::Service), "service");
        assert_eq!(
            describe_state(&StateManagement::Shared("core".to_string())),
            "shares the state 'core'"
        );
    }
}
