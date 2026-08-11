//! Checks that every example under `examples/` still says what it claims.
//!
//! Each pattern is a pair: `01-vanilla` is ordinary Terraform, and `02-envie` is
//! the same tree after `envie adopt`. Two things have to stay true, and both are
//! easy to break without noticing:
//!
//! - adoption still produces exactly the configuration checked in as `02-envie`,
//! - and it got there without touching a single `.tf` file.
//!
//! Terraform is never invoked, so these run without credentials or network
//! access.

use envie::commands::adopt::{AdoptCommand, AdoptOptions};
use envie::common::deployment::{PlanRequest, Planner};
use envie::common::project::Project;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// An example pair, and the adoption its `ADOPTION.md` documents.
struct Example {
    pattern: &'static str,
    project_name: &'static str,
    environments: &'static [&'static str],
}

const EXAMPLES: &[Example] = &[
    Example {
        pattern: "static-site",
        project_name: "envie-test-static-site",
        environments: &["prod"],
    },
    Example {
        pattern: "workspaces-multi-env",
        project_name: "envie-test-workspaces",
        environments: &["prod", "staging", "dev"],
    },
    Example {
        pattern: "multi-stack-remote-state",
        project_name: "envie-test-multistack",
        environments: &["prod"],
    },
    Example {
        pattern: "envs-per-directory",
        project_name: "envie-test-envdirs",
        environments: &["prod", "dev"],
    },
    Example {
        pattern: "partial-backend-config",
        project_name: "envie-test-partial",
        environments: &["prod", "staging"],
    },
    Example {
        pattern: "monorepo-scattered",
        project_name: "envie-test-monorepo",
        environments: &["prod"],
    },
];

fn examples_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples")
}

/// Adoption is reproducible: re-running it on the vanilla tree rebuilds the
/// configuration checked in beside it.
#[test]
fn every_example_matches_what_adoption_produces_today() {
    for example in EXAMPLES {
        let vanilla = examples_root().join(example.pattern).join("01-vanilla");
        let adopted = examples_root().join(example.pattern).join("02-envie");
        assert!(
            vanilla.is_dir() && adopted.is_dir(),
            "{} should have both halves of the pair",
            example.pattern
        );

        let temp = TempDir::new().unwrap();
        let root = temp.path().join(example.pattern);
        copy_tree(&vanilla, &root);

        AdoptCommand::new(root.clone())
            .execute(AdoptOptions {
                project_name: Some(example.project_name.to_string()),
                environments: example.environments.iter().map(|s| s.to_string()).collect(),
                dry_run: false,
                force: false,
                verbose: false,
            })
            .unwrap_or_else(|e| panic!("adopting {} failed: {}", example.pattern, e));

        for (name, expected) in envie_files(&adopted) {
            let produced = fs::read_to_string(root.join(&name)).unwrap_or_else(|_| {
                panic!(
                    "{}: adoption did not write {}",
                    example.pattern,
                    name.display()
                )
            });
            assert_eq!(
                produced,
                expected,
                "{}: {} is no longer what adoption produces. Re-run:\n  \
                 cd examples/{} && rm -rf 02-envie && cp -R 01-vanilla 02-envie && \
                 cd 02-envie && envie adopt --name {} {}",
                example.pattern,
                name.display(),
                example.pattern,
                example.project_name,
                example
                    .environments
                    .iter()
                    .map(|e| format!("--environment {}", e))
                    .collect::<Vec<_>>()
                    .join(" "),
            );
        }
    }
}

/// The claim the examples exist to make: adopting a repository does not edit it.
#[test]
fn no_example_had_to_change_its_terraform() {
    for example in EXAMPLES {
        let vanilla = terraform_files(&examples_root().join(example.pattern).join("01-vanilla"));
        let adopted = terraform_files(&examples_root().join(example.pattern).join("02-envie"));

        assert!(
            !vanilla.is_empty(),
            "{} has no Terraform to compare",
            example.pattern
        );
        assert_eq!(
            vanilla.keys().collect::<Vec<_>>(),
            adopted.keys().collect::<Vec<_>>(),
            "{}: the two halves hold different Terraform files",
            example.pattern
        );
        for (name, contents) in &vanilla {
            assert_eq!(
                adopted.get(name),
                Some(contents),
                "{}: {} differs between 01-vanilla and 02-envie, so adoption is not drop-in",
                example.pattern,
                name.display()
            );
        }
    }
}

/// Every adopted example can be planned, for the environments it declares and
/// for a new one. This is as far as it goes without credentials: it exercises
/// resolution, state paths and dependency wiring, but applies nothing.
#[test]
fn every_example_can_be_planned() {
    for example in EXAMPLES {
        let root = examples_root().join(example.pattern).join("02-envie");
        let project = Project::discover(&root)
            .unwrap_or_else(|e| panic!("{} is not a project: {}", example.pattern, e));
        let planner = Planner::new(project)
            .unwrap_or_else(|e| panic!("{}: planner failed: {}", example.pattern, e));

        for environment in example.environments.iter().chain(["pr-1"].iter()) {
            // Directories copied per environment build the same names, so a new
            // environment takes one at a time. The example's README says so too.
            let unit = (example.pattern == "envs-per-directory" && *environment == "pr-1")
                .then(|| "dev".to_string());

            let plan = planner
                .plan(&PlanRequest {
                    environment: environment.to_string(),
                    unit,
                    environment_overrides: HashMap::new(),
                    include_dependencies: true,
                    no_prompt: true,
                    verbose: false,
                })
                .unwrap_or_else(|e| {
                    panic!("{} could not plan {}: {}", example.pattern, environment, e)
                });

            assert!(
                !plan.units.is_empty(),
                "{}: planning {} produced no units",
                example.pattern,
                environment
            );
        }
    }
}

/// Deploying the environment a repository was adopted with must not rename
/// anything, so Envie may only pass variables the repository has no answer for.
#[test]
fn adopted_environments_keep_the_values_their_repository_already_has() {
    for example in EXAMPLES {
        let root = examples_root().join(example.pattern).join("02-envie");
        let project = Project::discover(&root).unwrap();
        let planner = Planner::new(project).unwrap();
        let environment = example.environments[0];

        let plan = planner
            .plan(&PlanRequest {
                environment: environment.to_string(),
                unit: None,
                environment_overrides: HashMap::new(),
                include_dependencies: true,
                no_prompt: true,
                verbose: false,
            })
            .unwrap();

        for unit in &plan.units {
            for (name, value) in &unit.vars {
                assert_eq!(
                    value, environment,
                    "{}: unit {} would be deployed to {} with {}={}, which is not what \
                     built the existing infrastructure",
                    example.pattern, unit.name, environment, name, value
                );
            }
        }
    }
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// The configuration files adoption is responsible for, by path from the tree root.
fn envie_files(root: &Path) -> BTreeMap<PathBuf, String> {
    collect(root, root, &|name| {
        name == "envie.yaml" || name == "workspace.envie.yaml"
    })
}

/// The repository's own Terraform. Files Envie generates at deploy time are left
/// out: they are gitignored, and whether one happens to be lying around says
/// nothing about whether adoption had to edit anything.
fn terraform_files(root: &Path) -> BTreeMap<PathBuf, String> {
    collect(root, root, &|name| {
        (name.ends_with(".tf") || name.ends_with(".tfvars") || name.ends_with(".tfbackend"))
            && !envie::common::tf_scan::is_envie_generated(name)
    })
}

fn collect(root: &Path, dir: &Path, wanted: &dyn Fn(&str) -> bool) -> BTreeMap<PathBuf, String> {
    let mut found = BTreeMap::new();
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if entry.file_type().unwrap().is_dir() {
            if name != ".terraform" {
                found.extend(collect(root, &path, wanted));
            }
        } else if wanted(&name) {
            let relative = path.strip_prefix(root).unwrap().to_path_buf();
            found.insert(relative, fs::read_to_string(&path).unwrap());
        }
    }
    found
}
