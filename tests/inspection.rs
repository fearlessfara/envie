//! Checks that `envie show` describes the whole project, from anywhere in it.
//!
//! It used to discover units starting from the current directory, so running it
//! inside a unit reported that unit alone, with an empty path and dependencies on
//! units it claimed did not exist. Anything that answers "what is here?" has to
//! answer the same way wherever it is run, or it teaches people the wrong shape of
//! their own repository.
//!
//! No Terraform is invoked.

use envie::commands::init::{InitCommand, InitOptions};
use envie::commands::show::{ShowCommand, ShowOptions};
use envie::common::project::Project;
use std::fs;
use tempfile::TempDir;

async fn scaffold() -> TempDir {
    let tmp = TempDir::new().unwrap();
    InitCommand::new(tmp.path().to_path_buf())
        .execute(InitOptions {
            name: Some("inspection".to_string()),
            description: None,
            no_prompt: true,
            verbose: false,
        })
        .await
        .expect("init failed");
    tmp
}

#[tokio::test]
async fn showing_from_inside_a_unit_still_finds_the_whole_project() {
    let tmp = scaffold().await;

    for directory in [tmp.path().to_path_buf(), tmp.path().join("units/api")] {
        let project = Project::discover(&directory).unwrap();
        let units = project.units().unwrap();
        let mut names: Vec<&str> = units
            .get_all_units()
            .iter()
            .map(|unit| unit.config.name.as_str())
            .collect();
        names.sort();

        assert_eq!(
            names,
            vec!["api", "db"],
            "running in {} should still see both units",
            directory.display()
        );

        ShowCommand::new(directory.clone())
            .execute(ShowOptions {
                unit: None,
                verbose: false,
            })
            .unwrap_or_else(|e| panic!("show failed in {}: {}", directory.display(), e));
    }
}

#[tokio::test]
async fn a_unit_that_does_not_exist_says_which_ones_do() {
    let tmp = scaffold().await;

    let error = ShowCommand::new(tmp.path().to_path_buf())
        .execute(ShowOptions {
            unit: Some("lambda".to_string()),
            verbose: false,
        })
        .unwrap_err();

    let message = error.to_string();
    assert!(message.contains("lambda"), "{message}");
    assert!(
        message.contains("api") && message.contains("db"),
        "the error should list the units there are: {message}"
    );
}

#[tokio::test]
async fn showing_a_single_unit_works_for_both_scaffolded_units() {
    let tmp = scaffold().await;

    for unit in ["db", "api"] {
        ShowCommand::new(tmp.path().to_path_buf())
            .execute(ShowOptions {
                unit: Some(unit.to_string()),
                verbose: false,
            })
            .unwrap_or_else(|e| panic!("show --unit {} failed: {}", unit, e));
    }
}

#[test]
fn a_project_with_no_units_explains_itself_rather_than_printing_nothing() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("workspace.envie.yaml"),
        "version: \"1.0\"\nproject:\n  name: empty\n",
    )
    .unwrap();

    let error = ShowCommand::new(tmp.path().to_path_buf())
        .execute(ShowOptions {
            unit: None,
            verbose: false,
        })
        .unwrap_err();

    assert!(
        error.to_string().contains("envie adopt"),
        "an empty project should point at how to get units: {error}"
    );
}
