//! Checks that `envie list` can answer "which environments exist?".
//!
//! Declared environments come out of the project file, but ephemeral ones exist
//! only because somebody deployed them, possibly from another machine. Envie has
//! to read that back out of the deployment records, and it has to keep working
//! when one of those records is missing or unreadable — an environment that is
//! hard to find is an environment nobody tears down.
//!
//! Everything here uses a local backend, so no credentials or network are needed.

use envie::commands::list::{summarise, EnvironmentKind};
use envie::common::manifest::{self, DeployedDependency, DeployedUnit, EnvironmentManifest};
use envie::common::project::Project;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

const PROJECT: &str = r#"version: "1.0"
project:
  name: acme
environments:
  ephemeral:
    naming_pattern: "{project}-{id}"
    backend:
      type: local
  stable:
    prod:
      workspace: default
      description: The real one
      backend:
        type: local
    staging:
      workspace: default
      backend:
        type: local
"#;

fn project() -> TempDir {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("workspace.envie.yaml"), PROJECT).unwrap();
    tmp
}

/// Record a deployment the way `envie deploy` does.
fn record(root: &Path, environment: &str, units: &[(&str, &[&str])]) {
    let project = Project::discover(root).unwrap();
    let resolved = project
        .resolver(environment)
        .resolve_environment(environment)
        .unwrap();

    let manifest = EnvironmentManifest {
        version: 1,
        project: project.name(),
        environment: environment.to_string(),
        workspace: resolved.workspace.clone(),
        updated_at: "2026-08-11T22:03:04Z".to_string(),
        units: units
            .iter()
            .map(|(name, dependencies)| {
                (
                    name.to_string(),
                    DeployedUnit {
                        path: format!("units/{}", name),
                        state_key: Some(format!("{}/terraform.tfstate", name)),
                        dependencies: dependencies
                            .iter()
                            .map(|unit| DeployedDependency {
                                unit: unit.to_string(),
                                environment: resolved.reference(),
                            })
                            .collect(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>(),
    };

    manifest::save(root, &resolved, manifest).unwrap();
}

#[test]
fn declared_environments_are_listed_before_anyone_deploys_them() {
    let tmp = project();

    let summary = summarise(tmp.path()).unwrap();

    let names: Vec<&str> = summary
        .environments
        .iter()
        .map(|environment| environment.name.as_str())
        .collect();
    assert_eq!(names, vec!["prod", "staging"]);
    assert!(
        summary.get("prod").unwrap().deployment.is_none(),
        "nothing has been deployed yet, and saying otherwise would be a lie"
    );
    assert_eq!(
        summary.get("prod").unwrap().description.as_deref(),
        Some("The real one")
    );
    assert!(summary.problems.is_empty(), "{:?}", summary.problems);
}

#[test]
fn an_ephemeral_environment_is_found_because_it_was_deployed() {
    let tmp = project();
    record(tmp.path(), "pr-1", &[("db", &[]), ("api", &["db"])]);

    let summary = summarise(tmp.path()).unwrap();

    let pr = summary
        .get("pr-1")
        .expect("a deployed environment should be listed");
    assert_eq!(pr.kind, EnvironmentKind::Ephemeral);
    assert_eq!(
        pr.workspace, "acme-pr-1",
        "the workspace should be the one it was deployed into"
    );
    let deployment = pr.deployment.as_ref().unwrap();
    assert_eq!(deployment.units, vec!["api", "db"]);
    assert_eq!(deployment.updated_at, "2026-08-11T22:03:04Z");
}

#[test]
fn deploying_a_declared_environment_does_not_duplicate_it() {
    let tmp = project();
    record(tmp.path(), "prod", &[("db", &[])]);

    let summary = summarise(tmp.path()).unwrap();

    let matching: Vec<_> = summary
        .environments
        .iter()
        .filter(|environment| environment.name == "prod")
        .collect();
    assert_eq!(matching.len(), 1, "prod is one environment, not two");
    assert_eq!(matching[0].kind, EnvironmentKind::Stable);
    assert_eq!(
        matching[0].deployment.as_ref().unwrap().units,
        vec!["db"],
        "a declared environment should still report what it has deployed"
    );
}

#[test]
fn an_unreadable_record_still_reveals_the_environment() {
    let tmp = project();
    let manifests = tmp.path().join(".envie/manifests");
    fs::create_dir_all(&manifests).unwrap();
    fs::write(manifests.join("pr-7.json"), "{ this is not json").unwrap();

    let summary = summarise(tmp.path()).unwrap();

    let pr = summary
        .get("pr-7")
        .expect("an environment with a damaged record is still out there");
    assert!(pr.deployment.is_none());
    assert_eq!(
        pr.workspace, "acme-pr-7",
        "with no record to read, the workspace comes from the naming pattern"
    );
    assert!(
        summary
            .problems
            .iter()
            .any(|problem| problem.contains("pr-7")),
        "an incomplete answer has to say so: {:?}",
        summary.problems
    );
}

#[test]
fn deleting_an_environment_takes_it_off_the_list() {
    let tmp = project();
    record(tmp.path(), "pr-1", &[("db", &[])]);
    let project = Project::discover(tmp.path()).unwrap();
    let resolved = project
        .resolver("pr-1")
        .resolve_environment("pr-1")
        .unwrap();

    manifest::remove(tmp.path(), &resolved).unwrap();

    let summary = summarise(tmp.path()).unwrap();
    assert!(summary.get("pr-1").is_none());
    assert!(
        summary.get("prod").is_some(),
        "deleting an ephemeral environment must not hide the declared ones"
    );
}

#[test]
fn the_json_form_is_shaped_for_scripts() {
    let tmp = project();
    record(tmp.path(), "pr-1", &[("api", &["db"])]);

    let summary = summarise(tmp.path()).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&summary).unwrap()).unwrap();

    assert_eq!(json["project"], "acme");
    let pr = json["environments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|environment| environment["name"] == "pr-1")
        .expect("pr-1 should be in the JSON");
    assert_eq!(pr["kind"], "ephemeral");
    assert_eq!(pr["deployment"]["units"][0], "api");
}
