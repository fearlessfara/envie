use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;
use serde_yaml::Value;

#[test]
fn test_scaffold_simple_template() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("scaffold")
        .arg("myservice")
        .arg("--template")
        .arg("simple")
        .arg("--no-prompt");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("✅ Unit 'myservice' created successfully!"));

    // Verify directory structure
    let unit_path = temp_dir.path().join("services").join("myservice");
    assert!(unit_path.exists(), "Unit directory should exist");
    assert!(unit_path.join("envie.yaml").exists(), "envie.yaml should exist");
    assert!(unit_path.join("main.tf").exists(), "main.tf should exist");
}

#[test]
fn test_scaffold_with_modules_template() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("scaffold")
        .arg("myservice")
        .arg("--template")
        .arg("with-modules")
        .arg("--module")
        .arg("lambda")
        .arg("--module")
        .arg("gateway")
        .arg("--no-prompt");

    cmd.assert().success();

    let unit_path = temp_dir.path().join("services").join("myservice");
    assert!(unit_path.join("modules").join("lambda").exists());
    assert!(unit_path.join("modules").join("gateway").exists());
    assert!(unit_path.join("modules").join("lambda").join("main.tf").exists());
    assert!(unit_path.join("modules").join("gateway").join("main.tf").exists());
}

#[test]
fn test_scaffold_networking_template() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("scaffold")
        .arg("network")
        .arg("--template")
        .arg("networking")
        .arg("--no-prompt");

    cmd.assert().success();

    let unit_path = temp_dir.path().join("services").join("network");
    assert!(unit_path.join("envie.yaml").exists());
    assert!(unit_path.join("modules").join("vpc").join("main.tf").exists());
    assert!(unit_path.join("modules").join("subnets").join("main.tf").exists());

    // Verify envie.yaml structure
    let yaml_content = fs::read_to_string(unit_path.join("envie.yaml")).unwrap();
    let yaml: Value = serde_yaml::from_str(&yaml_content).unwrap();

    assert_eq!(yaml["name"].as_str(), Some("network"));
    let modules = yaml["modules"].as_sequence().unwrap();
    assert_eq!(modules.len(), 2); // vpc and subnets
}

#[test]
fn test_scaffold_database_template() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("scaffold")
        .arg("data")
        .arg("--template")
        .arg("database")
        .arg("--no-prompt");

    cmd.assert().success();

    let unit_path = temp_dir.path().join("services").join("data");
    assert!(unit_path.join("envie.yaml").exists());
    assert!(unit_path.join("modules").join("dynamodb").join("main.tf").exists());

    // Verify state management is Dedicated
    let yaml_content = fs::read_to_string(unit_path.join("envie.yaml")).unwrap();
    assert!(yaml_content.contains("state_management: Dedicated"));
}

#[test]
fn test_scaffold_api_template() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("scaffold")
        .arg("auth")
        .arg("--template")
        .arg("api")
        .arg("--no-prompt");

    cmd.assert().success();

    let unit_path = temp_dir.path().join("services").join("auth");
    assert!(unit_path.join("envie.yaml").exists());
    assert!(unit_path.join("modules").join("lambda").join("main.tf").exists());
    assert!(unit_path.join("modules").join("gateway").join("main.tf").exists());

    // Verify gateway depends on lambda
    let yaml_content = fs::read_to_string(unit_path.join("envie.yaml")).unwrap();
    let yaml: Value = serde_yaml::from_str(&yaml_content).unwrap();

    let modules = yaml["modules"].as_sequence().unwrap();
    let gateway = modules.iter()
        .find(|m| m["name"].as_str() == Some("gateway"))
        .unwrap();

    let deps = gateway["dependencies"].as_sequence().unwrap();
    assert!(!deps.is_empty(), "Gateway should have dependencies");
}

#[test]
fn test_scaffold_compute_template() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("scaffold")
        .arg("worker")
        .arg("--template")
        .arg("compute")
        .arg("--no-prompt");

    cmd.assert().success();

    let unit_path = temp_dir.path().join("services").join("worker");
    assert!(unit_path.join("envie.yaml").exists());
    assert!(unit_path.join("modules").join("functions").join("main.tf").exists());
}

#[test]
fn test_scaffold_custom_path() {
    let temp_dir = TempDir::new().unwrap();
    let custom_path = temp_dir.path().join("custom").join("location").join("myservice");

    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("scaffold")
        .arg("myservice")
        .arg("--template")
        .arg("simple")
        .arg("--path")
        .arg(custom_path.to_str().unwrap())
        .arg("--no-prompt");

    cmd.assert().success();

    assert!(custom_path.exists(), "Custom path should exist");
    assert!(custom_path.join("envie.yaml").exists());
}

#[test]
fn test_scaffold_unit_already_exists() {
    let temp_dir = TempDir::new().unwrap();

    // Create first unit
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("scaffold")
        .arg("myservice")
        .arg("--template")
        .arg("simple")
        .arg("--no-prompt");
    cmd.assert().success();

    // Try to create same unit again
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("scaffold")
        .arg("myservice")
        .arg("--template")
        .arg("simple")
        .arg("--no-prompt");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn test_scaffold_invalid_template() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("scaffold")
        .arg("myservice")
        .arg("--template")
        .arg("invalid-template")
        .arg("--no-prompt");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Invalid template"));
}

#[test]
fn test_scaffold_verbose_output() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("scaffold")
        .arg("myservice")
        .arg("--template")
        .arg("simple")
        .arg("--no-prompt")
        .arg("--verbose");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("🚀 Creating new Envie unit"))
        .stdout(predicate::str::contains("📋 Using template"));
}

#[test]
fn test_scaffold_creates_valid_yaml() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("scaffold")
        .arg("testservice")
        .arg("--template")
        .arg("simple")
        .arg("--no-prompt");

    cmd.assert().success();

    let yaml_path = temp_dir.path().join("services").join("testservice").join("envie.yaml");
    let content = fs::read_to_string(yaml_path).unwrap();

    // Should be valid YAML
    let yaml: Value = serde_yaml::from_str(&content).expect("YAML should be valid");

    assert_eq!(yaml["name"].as_str(), Some("testservice"));
    assert!(yaml["description"].as_str().is_some());
}

#[test]
fn test_scaffold_terraform_files_have_variables() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("scaffold")
        .arg("myservice")
        .arg("--template")
        .arg("simple")
        .arg("--no-prompt");

    cmd.assert().success();

    let main_tf_path = temp_dir.path().join("services").join("myservice").join("main.tf");
    let content = fs::read_to_string(main_tf_path).unwrap();

    // Should have standard Envie variables
    assert!(content.contains("variable \"envie_workspace\""));
    assert!(content.contains("variable \"envie_environment\""));
}

#[test]
fn test_scaffold_networking_has_aws_resources() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("scaffold")
        .arg("network")
        .arg("--template")
        .arg("networking")
        .arg("--no-prompt");

    cmd.assert().success();

    let vpc_tf = temp_dir.path()
        .join("services").join("network")
        .join("modules").join("vpc")
        .join("main.tf");

    let content = fs::read_to_string(vpc_tf).unwrap();
    assert!(content.contains("aws_vpc"), "Should have aws_vpc resource");
    assert!(content.contains("output \"vpc_id\""), "Should have vpc_id output");
}

#[test]
fn test_scaffold_database_has_dynamodb() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("scaffold")
        .arg("data")
        .arg("--template")
        .arg("database")
        .arg("--no-prompt");

    cmd.assert().success();

    let dynamodb_tf = temp_dir.path()
        .join("services").join("data")
        .join("modules").join("dynamodb")
        .join("main.tf");

    let content = fs::read_to_string(dynamodb_tf).unwrap();
    assert!(content.contains("aws_dynamodb_table"), "Should have aws_dynamodb_table resource");
}

#[test]
fn test_scaffold_api_has_lambda() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("scaffold")
        .arg("auth")
        .arg("--template")
        .arg("api")
        .arg("--no-prompt");

    cmd.assert().success();

    let lambda_tf = temp_dir.path()
        .join("services").join("auth")
        .join("modules").join("lambda")
        .join("main.tf");

    let content = fs::read_to_string(lambda_tf).unwrap();
    assert!(content.contains("aws_lambda_function"), "Should have aws_lambda_function resource");
    assert!(content.contains("aws_iam_role"), "Should have aws_iam_role resource");
}

#[test]
fn test_scaffold_with_modules_default_names() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("scaffold")
        .arg("myservice")
        .arg("--template")
        .arg("with-modules")
        .arg("--no-prompt");

    cmd.assert().success();

    let unit_path = temp_dir.path().join("services").join("myservice");

    // Should create default modules (module1, module2)
    assert!(unit_path.join("modules").join("module1").exists());
    assert!(unit_path.join("modules").join("module2").exists());
}

#[test]
fn test_scaffold_module_dependency_chain() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("scaffold")
        .arg("myservice")
        .arg("--template")
        .arg("with-modules")
        .arg("--module")
        .arg("first")
        .arg("--module")
        .arg("second")
        .arg("--module")
        .arg("third")
        .arg("--no-prompt");

    cmd.assert().success();

    let yaml_path = temp_dir.path().join("services").join("myservice").join("envie.yaml");
    let content = fs::read_to_string(yaml_path).unwrap();
    let yaml: Value = serde_yaml::from_str(&content).unwrap();

    let modules = yaml["modules"].as_sequence().unwrap();
    assert_eq!(modules.len(), 3);

    // First module should have no dependencies
    let first = &modules[0];
    assert_eq!(first["name"].as_str(), Some("first"));
    assert!(first["dependencies"].as_sequence().unwrap().is_empty());

    // Second module should depend on first
    let second = &modules[1];
    assert_eq!(second["name"].as_str(), Some("second"));
    assert!(!second["dependencies"].as_sequence().unwrap().is_empty());

    // Third module should depend on second
    let third = &modules[2];
    assert_eq!(third["name"].as_str(), Some("third"));
    assert!(!third["dependencies"].as_sequence().unwrap().is_empty());
}

#[test]
fn test_scaffold_helpful_next_steps() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("scaffold")
        .arg("myservice")
        .arg("--template")
        .arg("simple")
        .arg("--no-prompt");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("🚀 Next steps:"))
        .stdout(predicate::str::contains("Customize the envie.yaml"))
        .stdout(predicate::str::contains("envie deploy --unit myservice"));
}
