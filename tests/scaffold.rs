//! Checks that `envie init` writes a project Envie can actually read.
//!
//! Scaffolding is easy to break silently: the configuration schema moves on, the
//! templates do not, and nobody notices because nobody runs `init` on a project
//! they already have. When that happens every command fails at the first parse,
//! which is the worst possible introduction to the tool.
//!
//! Terraform is never invoked, so these run without credentials or network
//! access.

use envie::commands::init::{InitCommand, InitOptions};
use envie::common::deployment::{PlanRequest, Planner};
use envie::common::project::Project;
use std::collections::HashMap;
use tempfile::TempDir;

async fn scaffold() -> TempDir {
    let tmp = TempDir::new().unwrap();
    InitCommand::new(tmp.path().to_path_buf())
        .execute(InitOptions {
            name: Some("scaffold-test".to_string()),
            description: Some("A scaffolded project".to_string()),
            no_prompt: true,
            verbose: false,
        })
        .await
        .expect("init failed");
    tmp
}

fn plan(root: &std::path::Path, environment: &str) -> envie::common::deployment::Plan {
    let project = Project::discover(root)
        .unwrap_or_else(|e| panic!("init wrote a project Envie cannot read: {}", e));
    Planner::new(project)
        .unwrap_or_else(|e| panic!("init wrote a project that cannot be planned: {}", e))
        .plan(&PlanRequest {
            environment: environment.to_string(),
            unit: None,
            environment_overrides: HashMap::new(),
            include_dependencies: true,
            no_prompt: true,
            verbose: false,
        })
        .unwrap_or_else(|e| panic!("a scaffolded project could not plan {}: {}", environment, e))
}

#[tokio::test]
async fn a_scaffolded_project_can_be_planned_for_both_kinds_of_environment() {
    let tmp = scaffold().await;

    for environment in ["pr-1", "prod"] {
        let plan = plan(tmp.path(), environment);
        let names: Vec<&str> = plan.units.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["db", "api"],
            "planning {} should deploy both scaffolded units, dependency first",
            environment
        );
    }
}

/// The point of the two scaffolded units is to show a dependency being wired, so
/// a scaffold where `api` does not read `db` demonstrates nothing.
#[tokio::test]
async fn the_scaffolded_dependency_is_wired_to_the_environment_being_deployed() {
    let tmp = scaffold().await;
    let plan = plan(tmp.path(), "pr-1");

    let api = plan.units.iter().find(|u| u.name == "api").unwrap();
    let dependency = api
        .dependencies
        .first()
        .expect("api should depend on db so the example shows remote state");

    assert_eq!(dependency.data_source_name, "db");
    assert!(
        dependency.state.config["key"].contains("pr-1"),
        "api should read the state of the environment being deployed, got {}",
        dependency.state.config["key"]
    );
}

/// Every environment must get its own state, or the scaffold teaches the one
/// thing that would lose someone's infrastructure.
#[tokio::test]
async fn each_environment_gets_its_own_state() {
    let tmp = scaffold().await;

    let ephemeral = plan(tmp.path(), "pr-1");
    let stable = plan(tmp.path(), "prod");

    for (a, b) in ephemeral.units.iter().zip(stable.units.iter()) {
        assert_ne!(
            a.target.config["key"], b.target.config["key"],
            "unit {} would write the same state in pr-1 and prod",
            a.name
        );
    }
}

#[tokio::test]
async fn scaffolding_does_not_overwrite_an_existing_project() {
    let tmp = scaffold().await;
    let config = tmp.path().join("workspace.envie.yaml");
    std::fs::write(&config, "version: \"1.0\"\nproject:\n  name: mine\n").unwrap();

    // `no_prompt` is what scripts and CI use, and there is nobody there to
    // answer, so the only safe answer is to refuse.
    let error = InitCommand::new(tmp.path().to_path_buf())
        .execute(InitOptions {
            name: Some("scaffold-test".to_string()),
            description: None,
            no_prompt: true,
            verbose: false,
        })
        .await
        .expect_err("init should refuse to overwrite an existing configuration");
    assert!(
        error.to_string().contains("already exists"),
        "the refusal should say what is in the way, got: {}",
        error
    );

    assert_eq!(
        std::fs::read_to_string(&config).unwrap(),
        "version: \"1.0\"\nproject:\n  name: mine\n",
        "init overwrote a configuration that was already there"
    );
}
