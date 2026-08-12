//! Checks that the documentation describes the tool that exists.
//!
//! Docs rot silently: a command gets renamed, a scaffold changes shape, a file is
//! deleted, and the guide keeps confidently describing the old one. Somebody
//! following it then concludes the tool is broken. These tests hold the docs to
//! the things that can be checked mechanically — commands, links, and the layout
//! `envie init` actually produces.

use clap::CommandFactory;
use envie::cli::args::Cli;
use envie::commands::init::{InitCommand, InitOptions};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn docs() -> Vec<(PathBuf, String)> {
    [
        "README.md",
        "QUICKSTART.md",
        "ENVIRONMENT_OVERRIDES.md",
        "CONTRIBUTING.md",
    ]
    .iter()
    .map(|name| {
        let path = repository_root().join(name);
        let body = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} should be readable: {}", path.display(), e));
        (path, body)
    })
    .collect()
}

#[test]
fn every_command_the_docs_mention_exists() {
    let known: HashSet<String> = Cli::command()
        .get_subcommands()
        .map(|command| command.get_name().to_string())
        .collect();

    for (path, body) in docs() {
        for candidate in body.split("envie ").skip(1) {
            let word = candidate
                .split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
                .next()
                .unwrap_or("");

            // Flags and `envie` on its own say nothing about a subcommand.
            if word.is_empty() || word.starts_with('-') {
                continue;
            }

            assert!(
                known.contains(word),
                "{} documents `envie {}`, which is not a command. Commands: {:?}",
                path.display(),
                word,
                known
            );
        }
    }
}

#[test]
fn the_crate_version_is_in_the_changelog() {
    let version = env!("CARGO_PKG_VERSION");
    let changelog = std::fs::read_to_string(repository_root().join("CHANGELOG.md"))
        .expect("CHANGELOG.md should be readable");
    let heading = format!("## [{version}]");
    assert!(
        changelog.contains(&heading),
        "CHANGELOG.md has no {heading} section; bump the changelog with Cargo.toml"
    );
}

#[test]
fn the_repository_has_a_license_file() {
    assert!(
        repository_root().join("LICENSE").exists(),
        "LICENSE is missing; Cargo.toml claims MIT"
    );
}

#[test]
fn the_docs_do_not_link_to_files_that_are_gone() {
    for (path, body) in docs() {
        for link in body.split("](").skip(1) {
            let target = link.split(')').next().unwrap_or("");
            if target.is_empty()
                || target.starts_with("http")
                || target.starts_with('#')
                || target.starts_with("mailto:")
            {
                continue;
            }

            let target = target.split('#').next().unwrap();
            assert!(
                repository_root().join(target).exists(),
                "{} links to {}, which does not exist",
                path.display(),
                target
            );
        }
    }
}

#[tokio::test]
async fn the_quickstart_describes_the_project_init_creates() {
    let tmp = TempDir::new().unwrap();
    InitCommand::new(tmp.path().to_path_buf())
        .execute(InitOptions {
            name: Some("myapp".to_string()),
            description: Some("My application infrastructure".to_string()),
            no_prompt: true,
            verbose: false,
        })
        .await
        .expect("init failed");

    let quickstart = std::fs::read_to_string(repository_root().join("QUICKSTART.md")).unwrap();

    // The guide walks through these paths by name, and the walkthrough is useless
    // the moment the scaffold stops producing them.
    for path in [
        "workspace.envie.yaml",
        "units/db/envie.yaml",
        "units/db/main.tf",
        "units/api/envie.yaml",
        "units/api/main.tf",
    ] {
        assert!(
            tmp.path().join(path).exists(),
            "init no longer creates {}, which QUICKSTART.md walks through",
            path
        );
        assert!(
            quickstart.contains(path) || quickstart.contains(parent_of(path)),
            "QUICKSTART.md does not mention {}",
            path
        );
    }
}

fn parent_of(path: &str) -> &str {
    Path::new(path)
        .parent()
        .and_then(|p| p.to_str())
        .filter(|p| !p.is_empty())
        .unwrap_or(path)
}
