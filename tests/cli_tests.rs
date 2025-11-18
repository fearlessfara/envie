use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;
use std::fs;

#[test]
fn test_version_flag() {
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.arg("--version");
    cmd.assert().success().stdout(predicate::str::contains("envie"));
}

#[test]
fn test_help_flag() {
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("QUICK START"))
        .stdout(predicate::str::contains("envie init"))
        .stdout(predicate::str::contains("envie deploy"));
}

#[test]
fn test_deploy_help() {
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.arg("deploy").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("EXAMPLES"))
        .stdout(predicate::str::contains("envie deploy --unit api --env dev-123"));
}

#[test]
fn test_plan_help() {
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.arg("plan").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Preview deployment"))
        .stdout(predicate::str::contains("alias for deploy --dry-run"));
}

#[test]
fn test_doctor_help() {
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.arg("doctor").arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("health checks"))
        .stdout(predicate::str::contains("CHECKS PERFORMED"));
}

#[test]
fn test_doctor_no_project() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("doctor");

    // Should fail but give helpful output
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Health checks failed"));
}

#[test]
fn test_list_no_project() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("list");

    // Should handle gracefully
    cmd.assert();
}

#[test]
fn test_init_creates_structure() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("init")
        .arg("--name")
        .arg("test-project")
        .arg("--no-prompt");

    cmd.assert().success();

    // Verify workspace.envie.yaml was created
    let workspace_path = temp_dir.path().join("workspace.envie.yaml");
    assert!(workspace_path.exists());

    let content = fs::read_to_string(workspace_path).unwrap();
    assert!(content.contains("test-project"));

    // Verify services directory was created
    assert!(temp_dir.path().join("services").exists());

    // Verify .gitignore was created
    assert!(temp_dir.path().join(".gitignore").exists());
}

#[test]
fn test_deploy_missing_env_flag() {
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.arg("deploy").arg("--unit").arg("api");

    // Should fail with error about missing --env
    cmd.assert().failure();
}

#[test]
fn test_plan_missing_env_flag() {
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.arg("plan").arg("--unit").arg("api");

    // Should fail with error about missing --env
    cmd.assert().failure();
}

#[test]
fn test_invalid_command() {
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.arg("nonexistent");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

#[test]
fn test_environment_override_format() {
    let temp_dir = TempDir::new().unwrap();

    // Create a minimal valid project
    fs::create_dir_all(temp_dir.path().join("services/api")).unwrap();
    fs::write(
        temp_dir.path().join("workspace.envie.yaml"),
        "version: '1.0'\nproject:\n  name: test\nenvironments:\n  ephemeral:\n    naming_pattern: test-{id}\n    backend:\n      type: s3\n      config:\n        bucket: test\n        region: us-east-1\n        key_pattern: test\n        dynamodb_table: test\n        encrypt: 'true'",
    )
    .unwrap();
    fs::write(
        temp_dir.path().join("services/api/envie.yaml"),
        "name: api\ndescription: test\nunit_type: service\nstate_management: dedicated\ndependencies: []",
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("deploy")
        .arg("--unit")
        .arg("api")
        .arg("--env")
        .arg("test")
        .arg("-E")
        .arg("invalid-format");  // Missing colon

    // Should fail with helpful error message
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Invalid environment override format"))
        .stderr(predicate::str::contains("Expected format"))
        .stderr(predicate::str::contains("database:stable.sandbox"));
}

#[test]
fn test_show_command() {
    let temp_dir = TempDir::new().unwrap();

    // Create a minimal project
    fs::create_dir_all(temp_dir.path().join("services/api")).unwrap();
    fs::write(
        temp_dir.path().join("workspace.envie.yaml"),
        "version: '1.0'\nproject:\n  name: test",
    )
    .unwrap();
    fs::write(
        temp_dir.path().join("services/api/envie.yaml"),
        "name: api\ndescription: API service\nunit_type: service\nstate_management: dedicated\ndependencies: []",
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("show");

    cmd.assert().success();
}

#[test]
fn test_unit_not_found_error_message() {
    let temp_dir = TempDir::new().unwrap();

    // Create a minimal project
    fs::create_dir_all(temp_dir.path().join("services/api")).unwrap();
    fs::write(
        temp_dir.path().join("workspace.envie.yaml"),
        "version: '1.0'\nproject:\n  name: test\nenvironments:\n  ephemeral:\n    naming_pattern: test-{id}\n    backend:\n      type: s3\n      config:\n        bucket: test\n        region: us-east-1\n        key_pattern: test\n        dynamodb_table: test\n        encrypt: 'true'\n  stable: {}",
    )
    .unwrap();
    fs::write(
        temp_dir.path().join("services/api/envie.yaml"),
        "name: api\ndescription: test\nunit_type: service\nstate_management: dedicated\ndependencies: []",
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("deploy")
        .arg("--unit")
        .arg("ap")  // Typo - should suggest "api"
        .arg("--env")
        .arg("test");

    // Should fail with fuzzy matching suggestions
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Unit 'ap' not found"))
        .stderr(predicate::str::contains("Did you mean"))
        .stderr(predicate::str::contains("api"));
}
