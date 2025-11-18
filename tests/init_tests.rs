use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;
use serde_yaml::Value;

#[test]
fn test_init_basic_no_prompt() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("init")
        .arg("--name")
        .arg("test-project")
        .arg("--no-prompt");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("✅ Envie project initialized successfully!"));

    // Verify workspace.envie.yaml was created
    let workspace_path = temp_dir.path().join("workspace.envie.yaml");
    assert!(workspace_path.exists(), "workspace.envie.yaml should exist");

    let content = fs::read_to_string(workspace_path).unwrap();
    assert!(content.contains("test-project"), "workspace.envie.yaml should contain project name");
}

#[test]
fn test_init_with_description() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("init")
        .arg("--name")
        .arg("my-app")
        .arg("--description")
        .arg("My awesome infrastructure")
        .arg("--no-prompt");

    cmd.assert().success();

    // Verify workspace.envie.yaml contains both name and description
    let workspace_path = temp_dir.path().join("workspace.envie.yaml");
    let content = fs::read_to_string(workspace_path).unwrap();
    assert!(content.contains("my-app"));
    assert!(content.contains("My awesome infrastructure"));
}

#[test]
fn test_init_creates_services_directory() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("init")
        .arg("--name")
        .arg("test")
        .arg("--no-prompt");

    cmd.assert().success();

    // Verify services directory structure
    let services_dir = temp_dir.path().join("services");
    assert!(services_dir.exists(), "services directory should exist");
    assert!(services_dir.is_dir(), "services should be a directory");

    // Verify example services were created
    assert!(services_dir.join("networking").exists(), "networking service should exist");
    assert!(services_dir.join("database").exists(), "database service should exist");
    assert!(services_dir.join("api").exists(), "api service should exist");
}

#[test]
fn test_init_creates_networking_service_structure() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("init").arg("--name").arg("test").arg("--no-prompt");
    cmd.assert().success();

    let networking_dir = temp_dir.path().join("services").join("networking");

    // Verify envie.yaml exists
    assert!(networking_dir.join("envie.yaml").exists(), "networking envie.yaml should exist");

    // Verify modules directory
    let modules_dir = networking_dir.join("modules");
    assert!(modules_dir.exists(), "modules directory should exist");
    assert!(modules_dir.join("vpc").exists(), "vpc module should exist");
    assert!(modules_dir.join("subnets").exists(), "subnets module should exist");
    assert!(modules_dir.join("security-groups").exists(), "security-groups module should exist");

    // Verify main.tf files exist in modules
    assert!(modules_dir.join("vpc").join("main.tf").exists(), "vpc/main.tf should exist");
    assert!(modules_dir.join("subnets").join("main.tf").exists(), "subnets/main.tf should exist");
    assert!(modules_dir.join("security-groups").join("main.tf").exists(), "security-groups/main.tf should exist");
}

#[test]
fn test_init_creates_database_service_structure() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("init").arg("--name").arg("test").arg("--no-prompt");
    cmd.assert().success();

    let database_dir = temp_dir.path().join("services").join("database");

    // Verify envie.yaml exists
    assert!(database_dir.join("envie.yaml").exists(), "database envie.yaml should exist");

    // Verify modules
    let modules_dir = database_dir.join("modules");
    assert!(modules_dir.join("dynamodb").exists(), "dynamodb module should exist");
    assert!(modules_dir.join("rds").exists(), "rds module should exist");

    // Verify main.tf files
    assert!(modules_dir.join("dynamodb").join("main.tf").exists(), "dynamodb/main.tf should exist");
    assert!(modules_dir.join("rds").join("main.tf").exists(), "rds/main.tf should exist");
}

#[test]
fn test_init_creates_api_service_structure() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("init").arg("--name").arg("test").arg("--no-prompt");
    cmd.assert().success();

    let api_dir = temp_dir.path().join("services").join("api");

    // Verify envie.yaml exists
    assert!(api_dir.join("envie.yaml").exists(), "api envie.yaml should exist");

    // Verify modules
    let modules_dir = api_dir.join("modules");
    assert!(modules_dir.join("lambda").exists(), "lambda module should exist");
    assert!(modules_dir.join("step-functions").exists(), "step-functions module should exist");
    assert!(modules_dir.join("gateway").exists(), "gateway module should exist");

    // Verify main.tf files
    assert!(modules_dir.join("lambda").join("main.tf").exists(), "lambda/main.tf should exist");
    assert!(modules_dir.join("step-functions").join("main.tf").exists(), "step-functions/main.tf should exist");
    assert!(modules_dir.join("gateway").join("main.tf").exists(), "gateway/main.tf should exist");
}

#[test]
fn test_init_creates_valid_workspace_yaml() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("init")
        .arg("--name")
        .arg("valid-project")
        .arg("--description")
        .arg("A valid test project")
        .arg("--no-prompt");
    cmd.assert().success();

    let workspace_path = temp_dir.path().join("workspace.envie.yaml");
    let content = fs::read_to_string(workspace_path).unwrap();

    // Parse YAML to ensure it's valid
    let yaml: Value = serde_yaml::from_str(&content).expect("workspace.envie.yaml should be valid YAML");

    // Verify structure
    assert_eq!(yaml["version"].as_str(), Some("1.0"), "version should be 1.0");
    assert_eq!(yaml["project"]["name"].as_str(), Some("valid-project"), "project name should match");
    assert_eq!(yaml["project"]["description"].as_str(), Some("A valid test project"), "description should match");

    // Verify services are listed
    let services = yaml["services"].as_sequence().expect("services should be a sequence");
    assert_eq!(services.len(), 3, "should have 3 services");

    let service_names: Vec<String> = services.iter()
        .filter_map(|s| s["name"].as_str())
        .map(|s| s.to_string())
        .collect();

    assert!(service_names.contains(&"networking".to_string()), "should have networking service");
    assert!(service_names.contains(&"database".to_string()), "should have database service");
    assert!(service_names.contains(&"api".to_string()), "should have api service");
}

#[test]
fn test_init_creates_valid_unit_configs() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("init").arg("--name").arg("test").arg("--no-prompt");
    cmd.assert().success();

    // Test networking service config
    let networking_yaml = temp_dir.path().join("services").join("networking").join("envie.yaml");
    let content = fs::read_to_string(networking_yaml).unwrap();
    let yaml: Value = serde_yaml::from_str(&content).expect("networking envie.yaml should be valid");

    assert_eq!(yaml["name"].as_str(), Some("networking"));
    let modules = yaml["modules"].as_sequence().expect("should have modules");
    assert_eq!(modules.len(), 3, "networking should have 3 modules");

    // Test database service config
    let database_yaml = temp_dir.path().join("services").join("database").join("envie.yaml");
    let content = fs::read_to_string(database_yaml).unwrap();
    let yaml: Value = serde_yaml::from_str(&content).expect("database envie.yaml should be valid");

    assert_eq!(yaml["name"].as_str(), Some("database"));
    let modules = yaml["modules"].as_sequence().expect("should have modules");
    assert_eq!(modules.len(), 2, "database should have 2 modules");

    // Verify database has dependency on networking
    let dependencies = yaml["dependencies"].as_sequence().expect("should have dependencies");
    assert!(!dependencies.is_empty(), "database should have dependencies");

    // Test API service config
    let api_yaml = temp_dir.path().join("services").join("api").join("envie.yaml");
    let content = fs::read_to_string(api_yaml).unwrap();
    let yaml: Value = serde_yaml::from_str(&content).expect("api envie.yaml should be valid");

    assert_eq!(yaml["name"].as_str(), Some("api"));
    let modules = yaml["modules"].as_sequence().expect("should have modules");
    assert_eq!(modules.len(), 3, "api should have 3 modules");

    // Verify API has dependencies on database and networking
    let dependencies = yaml["dependencies"].as_sequence().expect("should have dependencies");
    assert_eq!(dependencies.len(), 2, "api should have 2 dependencies");
}

#[test]
fn test_init_creates_terraform_files() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("init").arg("--name").arg("test").arg("--no-prompt");
    cmd.assert().success();

    // Check VPC main.tf
    let vpc_main_tf = temp_dir.path()
        .join("services").join("networking")
        .join("modules").join("vpc")
        .join("main.tf");

    let content = fs::read_to_string(vpc_main_tf).unwrap();
    assert!(content.contains("vpc Module"), "should contain module header");
    assert!(content.contains("resource \"null_resource\""), "should contain resource");
    assert!(content.contains("output \"example_output\""), "should contain output");

    // Check Lambda main.tf
    let lambda_main_tf = temp_dir.path()
        .join("services").join("api")
        .join("modules").join("lambda")
        .join("main.tf");

    let content = fs::read_to_string(lambda_main_tf).unwrap();
    assert!(content.contains("lambda Module"), "should contain module header");
}

#[test]
fn test_init_creates_gitignore() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("init").arg("--name").arg("test").arg("--no-prompt");
    cmd.assert().success();

    let gitignore_path = temp_dir.path().join(".gitignore");
    assert!(gitignore_path.exists(), ".gitignore should exist");

    let content = fs::read_to_string(gitignore_path).unwrap();
    assert!(content.contains("*.envie.tf"), ".gitignore should contain *.envie.tf");
    assert!(content.contains(".terraform/"), ".gitignore should contain .terraform/");
    assert!(content.contains("*.tfstate"), ".gitignore should contain *.tfstate");
}

#[test]
fn test_init_updates_existing_gitignore() {
    let temp_dir = TempDir::new().unwrap();

    // Create existing .gitignore
    let gitignore_path = temp_dir.path().join(".gitignore");
    fs::write(&gitignore_path, "node_modules/\n.env\n").unwrap();

    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("init").arg("--name").arg("test").arg("--no-prompt");
    cmd.assert().success();

    let content = fs::read_to_string(gitignore_path).unwrap();

    // Should preserve existing entries
    assert!(content.contains("node_modules/"), "should preserve existing entries");
    assert!(content.contains(".env"), "should preserve existing entries");

    // Should add Envie entries
    assert!(content.contains("*.envie.tf"), "should add Envie entries");
    assert!(content.contains(".terraform/"), "should add Terraform entries");
}

#[test]
fn test_init_creates_readme() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("init")
        .arg("--name")
        .arg("my-project")
        .arg("--description")
        .arg("My awesome project")
        .arg("--no-prompt");
    cmd.assert().success();

    let readme_path = temp_dir.path().join("README.md");
    assert!(readme_path.exists(), "README.md should exist");

    let content = fs::read_to_string(readme_path).unwrap();
    assert!(content.contains("# my-project"), "README should contain project name as header");
    assert!(content.contains("My awesome project"), "README should contain description");
    assert!(content.contains("## Project Structure"), "README should have structure section");
    assert!(content.contains("## Quick Start"), "README should have quick start");
    assert!(content.contains("envie deploy"), "README should mention envie deploy");
}

#[test]
fn test_init_with_verbose() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("init")
        .arg("--name")
        .arg("test")
        .arg("--no-prompt")
        .arg("--verbose");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("🚀 Initializing Envie project"));
}

#[test]
fn test_init_already_initialized_no_prompt() {
    let temp_dir = TempDir::new().unwrap();

    // First init
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("init").arg("--name").arg("first").arg("--no-prompt");
    cmd.assert().success();

    // Second init (should proceed without prompt since --no-prompt is set)
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("init").arg("--name").arg("second").arg("--no-prompt");
    cmd.assert().success();

    // Verify it was overwritten with new name
    let workspace_path = temp_dir.path().join("workspace.envie.yaml");
    let content = fs::read_to_string(workspace_path).unwrap();
    assert!(content.contains("second"), "should be updated with new project name");
}

#[test]
fn test_init_dependency_graph_is_valid() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("init").arg("--name").arg("test").arg("--no-prompt");
    cmd.assert().success();

    // Verify networking has no service-level dependencies
    let networking_yaml = temp_dir.path().join("services").join("networking").join("envie.yaml");
    let content = fs::read_to_string(networking_yaml).unwrap();
    let yaml: Value = serde_yaml::from_str(&content).unwrap();
    let deps = yaml["dependencies"].as_sequence().unwrap();
    assert!(deps.is_empty(), "networking should have no service dependencies");

    // Verify database depends on networking
    let database_yaml = temp_dir.path().join("services").join("database").join("envie.yaml");
    let content = fs::read_to_string(database_yaml).unwrap();
    let yaml: Value = serde_yaml::from_str(&content).unwrap();
    let deps = yaml["dependencies"].as_sequence().unwrap();
    assert_eq!(deps.len(), 1, "database should have 1 dependency");
    assert!(deps[0].as_str().unwrap().contains("networking"), "database should depend on networking");

    // Verify API depends on both database and networking
    let api_yaml = temp_dir.path().join("services").join("api").join("envie.yaml");
    let content = fs::read_to_string(api_yaml).unwrap();
    let yaml: Value = serde_yaml::from_str(&content).unwrap();
    let deps = yaml["dependencies"].as_sequence().unwrap();
    assert_eq!(deps.len(), 2, "api should have 2 dependencies");
}

#[test]
fn test_init_module_dependencies_are_valid() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("init").arg("--name").arg("test").arg("--no-prompt");
    cmd.assert().success();

    // Check networking modules dependencies
    let networking_yaml = temp_dir.path().join("services").join("networking").join("envie.yaml");
    let content = fs::read_to_string(networking_yaml).unwrap();
    let yaml: Value = serde_yaml::from_str(&content).unwrap();
    let modules = yaml["modules"].as_sequence().unwrap();

    // VPC should have no dependencies
    let vpc = &modules[0];
    assert_eq!(vpc["name"].as_str(), Some("vpc"));
    let vpc_deps = vpc["dependencies"].as_sequence().unwrap();
    assert!(vpc_deps.is_empty(), "vpc should have no dependencies");

    // Subnets should depend on VPC
    let subnets = &modules[1];
    assert_eq!(subnets["name"].as_str(), Some("subnets"));
    let subnet_deps = subnets["dependencies"].as_sequence().unwrap();
    assert_eq!(subnet_deps.len(), 1, "subnets should have 1 dependency");
    assert!(subnet_deps[0]["path"].as_str().unwrap().contains("vpc"), "subnets should depend on vpc");
}

#[test]
fn test_init_state_management_configuration() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("init").arg("--name").arg("test").arg("--no-prompt");
    cmd.assert().success();

    // Check networking uses Service state management
    let networking_yaml = temp_dir.path().join("services").join("networking").join("envie.yaml");
    let content = fs::read_to_string(networking_yaml).unwrap();
    assert!(content.contains("state_management: Service"), "networking should use Service state management");

    // Check database uses Dedicated state management
    let database_yaml = temp_dir.path().join("services").join("database").join("envie.yaml");
    let content = fs::read_to_string(database_yaml).unwrap();
    assert!(content.contains("state_management: Dedicated"), "database should use Dedicated state management");
}

#[test]
fn test_init_cli_output_helpful() {
    let temp_dir = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.current_dir(temp_dir.path());
    cmd.arg("init").arg("--name").arg("test").arg("--no-prompt");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("✅ Envie project initialized successfully!"))
        .stdout(predicate::str::contains("📁 Project structure created:"))
        .stdout(predicate::str::contains("workspace.envie.yaml"))
        .stdout(predicate::str::contains("services/"))
        .stdout(predicate::str::contains("🚀 Next steps:"))
        .stdout(predicate::str::contains("envie deploy"));
}
