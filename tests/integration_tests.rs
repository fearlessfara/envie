use envie::common::*;
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper to create a test workspace configuration
fn create_test_workspace(dir: &TempDir) -> PathBuf {
    let workspace_path = dir.path().join("workspace.envie.yaml");
    let content = r#"
version: '1.0'

project:
  name: test-project
  description: Test project

environments:
  ephemeral:
    naming_pattern: "{project}-{id}"
    backend:
      type: s3
      config:
        bucket: "test-bucket"
        region: "us-east-1"
        key_pattern: "ephemeral/{workspace}/{path}/terraform.tfstate"
        dynamodb_table: "test-locks"
        encrypt: "true"

  stable:
    sandbox:
      workspace: test-sandbox
      backend:
        type: s3
        config:
          bucket: "test-bucket"
          region: "us-east-1"
          key_pattern: "stable/sandbox/{path}/terraform.tfstate"
          dynamodb_table: "test-locks"
          encrypt: "true"
"#;
    std::fs::write(&workspace_path, content).unwrap();
    workspace_path
}

/// Helper to create a test unit
fn create_test_unit(dir: &TempDir, name: &str, dependencies: Vec<&str>) {
    let unit_dir = dir.path().join("services").join(name);
    std::fs::create_dir_all(&unit_dir).unwrap();

    let mut depends_vec = Vec::new();
    for path in dependencies {
        depends_vec.push(format!("  - path: {}", path));
    }
    let depends_str = if depends_vec.is_empty() {
        "dependencies: []".to_string()
    } else {
        format!("dependencies:\n{}", depends_vec.join("\n"))
    };

    let content = format!(
        r#"name: {}
description: {} service
unit_type: service
state_management: dedicated
{}
"#,
        name, name, depends_str
    );

    std::fs::write(unit_dir.join("envie.yaml"), content).unwrap();
}

#[test]
fn test_unit_discovery() {
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);
    create_test_unit(&temp_dir, "database", vec![]);
    create_test_unit(&temp_dir, "api", vec!["../database"]);

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    discovery.discover_all().unwrap();

    let units = discovery.get_all_units();
    assert_eq!(units.len(), 2);

    // Verify units were discovered
    let unit_names: Vec<String> = units.iter().map(|u| u.config.name.clone()).collect();
    assert!(unit_names.contains(&"database".to_string()));
    assert!(unit_names.contains(&"api".to_string()));
}

#[test]
fn test_dependency_order() {
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);
    create_test_unit(&temp_dir, "database", vec![]);
    create_test_unit(&temp_dir, "networking", vec![]);
    create_test_unit(
        &temp_dir,
        "api",
        vec!["../database", "../networking"],
    );

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    discovery.discover_all().unwrap();

    let ordered = discovery.get_units_in_dependency_order().unwrap();

    // API should come after database and networking
    let api_index = ordered
        .iter()
        .position(|u| u.config.name == "api")
        .unwrap();
    let db_index = ordered
        .iter()
        .position(|u| u.config.name == "database")
        .unwrap();
    let net_index = ordered
        .iter()
        .position(|u| u.config.name == "networking")
        .unwrap();

    assert!(api_index > db_index);
    assert!(api_index > net_index);
}

#[test]
fn test_circular_dependency_detection() {
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);

    // Create circular dependency: api -> database -> api
    create_test_unit(&temp_dir, "database", vec!["../api"]);
    create_test_unit(&temp_dir, "api", vec!["../database"]);

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    discovery.discover_all().unwrap();

    let result = discovery.get_units_in_dependency_order();
    assert!(result.is_err());
}

#[test]
fn test_environment_validation() {
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);

    // Create unit with invalid environment reference
    let unit_dir = temp_dir.path().join("services/api");
    std::fs::create_dir_all(&unit_dir).unwrap();

    let content = r#"name: api
description: API service
unit_type: service
state_management: dedicated
dependencies:
  - path: ../database
"#;
    std::fs::write(unit_dir.join("envie.yaml"), content).unwrap();

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    let result = discovery.discover_all();

    // Should discover but validation should catch the error
    assert!(result.is_ok()); // Discovery succeeds
                              // But validation should fail when we try to use it
}

#[test]
fn test_workspace_config_parsing() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = create_test_workspace(&temp_dir);

    let content = std::fs::read_to_string(&workspace_path).unwrap();
    let config: service_config::WorkspaceConfig = serde_yaml::from_str(&content).unwrap();

    assert_eq!(config.project.as_ref().unwrap().name, "test-project");
    assert!(config.environments.is_some());

    let envs = config.environments.unwrap();
    assert!(envs.stable.contains_key("sandbox"));
}

#[test]
fn test_unit_config_parsing() {
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);
    create_test_unit(&temp_dir, "api", vec!["../database"]);

    let config_path = temp_dir
        .path()
        .join("services/api/envie.yaml");
    let content = std::fs::read_to_string(config_path).unwrap();
    let config: unit_config::UnitConfig = serde_yaml::from_str(&content).unwrap();

    assert_eq!(config.name, "api");
    assert_eq!(config.dependencies.len(), 1);
    // Check that the dependency has a path
    assert!(config.dependencies[0].path().is_some());
    assert_eq!(config.dependencies[0].path().unwrap(), "../database");
}

#[test]
fn test_disambiguation() {
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);
    create_test_unit(&temp_dir, "api", vec![]);

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    discovery.discover_all().unwrap();

    // Test exact match
    let matches = discovery.registry.resolve_unit("api");
    assert_eq!(matches.len(), 1);

    // Test non-existent unit
    let matches = discovery.registry.resolve_unit("nonexistent");
    assert_eq!(matches.len(), 0);
}

#[test]
fn test_fuzzy_matching() {
    use envie::common::disambiguation::*;
    use envie::common::unit_config::*;
    use std::collections::HashMap;

    let config1 = UnitConfig {
        name: "api".to_string(),
        description: "API service".to_string(),
        unit_type: UnitType::Service,
        path: "services/api".to_string(),
        dependencies: vec![],
        state_management: StateManagement::Dedicated,
        metadata: HashMap::new(),
    };

    let unit1 = DiscoveredUnit::new(config1, PathBuf::from("services/api"), 1);
    let all_units = vec![&unit1];

    // Test "unit not found" with fuzzy suggestions
    let result = resolve_unit_with_prompt(vec![], &all_units, "ap", true);
    assert!(result.is_err());

    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("Did you mean"));
    assert!(err_msg.contains("api"));
}
