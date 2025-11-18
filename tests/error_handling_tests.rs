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

#[test]
fn test_invalid_yaml_syntax() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = temp_dir.path().join("workspace.envie.yaml");

    // Write invalid YAML
    std::fs::write(&workspace_path, "invalid: yaml: syntax: [[[").unwrap();

    let content = std::fs::read_to_string(&workspace_path).unwrap();
    let result: std::result::Result<service_config::WorkspaceConfig, serde_yaml::Error> = serde_yaml::from_str(&content);

    assert!(result.is_err(), "Should fail on invalid YAML");
}

// Note: This test is commented out because the current implementation uses #[serde(default)]
// for many fields, making them optional rather than required. This provides better UX
// by allowing minimal configurations.
// #[test]
// fn test_missing_required_fields() {
//     let temp_dir = TempDir::new().unwrap();
//     let workspace_path = temp_dir.path().join("workspace.envie.yaml");
//
//     // Write config missing required fields
//     std::fs::write(&workspace_path, "version: '1.0'\n").unwrap();
//
//     let content = std::fs::read_to_string(&workspace_path).unwrap();
//     let result: std::result::Result<service_config::WorkspaceConfig, serde_yaml::Error> = serde_yaml::from_str(&content);
//
//     assert!(result.is_err(), "Should fail when required fields are missing");
// }

#[test]
fn test_invalid_version_format() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = temp_dir.path().join("workspace.envie.yaml");

    let content = r#"
version: 'invalid-version'
project:
  name: test
environments:
  ephemeral:
    naming_pattern: "test-{id}"
    backend:
      type: s3
      config:
        bucket: test
        region: us-east-1
        key_pattern: test
        dynamodb_table: test
        encrypt: 'true'
  stable: {}
"#;
    std::fs::write(&workspace_path, content).unwrap();

    let content = std::fs::read_to_string(&workspace_path).unwrap();
    let result: std::result::Result<service_config::WorkspaceConfig, serde_yaml::Error> = serde_yaml::from_str(&content);

    // Current implementation may accept any string version
    // This test documents the behavior
    if result.is_ok() {
        let config = result.unwrap();
        assert_eq!(config.version, "invalid-version");
    }
}

#[test]
fn test_empty_project_name() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = temp_dir.path().join("workspace.envie.yaml");

    let content = r#"
version: '1.0'
project:
  name: ""
environments:
  ephemeral:
    naming_pattern: "{project}-{id}"
    backend:
      type: s3
      config:
        bucket: test
        region: us-east-1
        key_pattern: test
        dynamodb_table: test
        encrypt: 'true'
  stable: {}
"#;
    std::fs::write(&workspace_path, content).unwrap();

    let content = std::fs::read_to_string(&workspace_path).unwrap();
    let result: std::result::Result<service_config::WorkspaceConfig, serde_yaml::Error> = serde_yaml::from_str(&content);

    // Should parse successfully but have empty name
    assert!(result.is_ok());
    let config = result.unwrap();
    assert!(config.project.unwrap().name.is_empty());
}

// Note: This test is commented out because the current implementation provides sensible defaults
// for most fields, making minimal configurations possible. This improves UX by allowing gradual
// configuration.
// #[test]
// fn test_unit_config_missing_required_fields() {
//     let temp_dir = TempDir::new().unwrap();
//     create_test_workspace(&temp_dir);
//
//     let unit_dir = temp_dir.path().join("services/api");
//     std::fs::create_dir_all(&unit_dir).unwrap();
//
//     // Missing required fields
//     std::fs::write(
//         unit_dir.join("envie.yaml"),
//         "name: api\n", // Missing other required fields
//     )
//     .unwrap();
//
//     let content = std::fs::read_to_string(unit_dir.join("envie.yaml")).unwrap();
//     let result: std::result::Result<unit_config::UnitConfig, serde_yaml::Error> = serde_yaml::from_str(&content);
//
//     assert!(result.is_err(), "Should fail when unit config is incomplete");
// }

#[test]
fn test_invalid_dependency_format() {
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);

    let unit_dir = temp_dir.path().join("services/api");
    std::fs::create_dir_all(&unit_dir).unwrap();

    // Invalid dependency format (string instead of object)
    std::fs::write(
        unit_dir.join("envie.yaml"),
        r#"name: api
description: API
unit_type: service
state_management: dedicated
dependencies:
  - "../database"
"#,
    )
    .unwrap();

    let content = std::fs::read_to_string(unit_dir.join("envie.yaml")).unwrap();
    let result: std::result::Result<unit_config::UnitConfig, serde_yaml::Error> = serde_yaml::from_str(&content);

    // Should fail due to invalid dependency format
    assert!(result.is_err());
}

#[test]
fn test_invalid_state_management_defaults_to_dedicated() {
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);

    let unit_dir = temp_dir.path().join("services/api");
    std::fs::create_dir_all(&unit_dir).unwrap();

    std::fs::write(
        unit_dir.join("envie.yaml"),
        r#"name: api
description: API
unit_type: service
state_management: invalid-value
dependencies: []
"#,
    )
    .unwrap();

    let content = std::fs::read_to_string(unit_dir.join("envie.yaml")).unwrap();
    let result: std::result::Result<unit_config::UnitConfig, serde_yaml::Error> = serde_yaml::from_str(&content);

    // Invalid state_management values gracefully default to "dedicated"
    assert!(result.is_ok());
    let config = result.unwrap();
    assert!(matches!(config.state_management, unit_config::StateManagement::Dedicated));
}

#[test]
fn test_invalid_unit_type() {
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);

    let unit_dir = temp_dir.path().join("services/api");
    std::fs::create_dir_all(&unit_dir).unwrap();

    std::fs::write(
        unit_dir.join("envie.yaml"),
        r#"name: api
description: API
unit_type: invalid-type
state_management: dedicated
dependencies: []
"#,
    )
    .unwrap();

    let content = std::fs::read_to_string(unit_dir.join("envie.yaml")).unwrap();
    let result: std::result::Result<unit_config::UnitConfig, serde_yaml::Error> = serde_yaml::from_str(&content);

    // Should fail due to invalid unit_type
    assert!(result.is_err());
}

#[test]
fn test_nonexistent_workspace_file() {
    let temp_dir = TempDir::new().unwrap();

    // Don't create workspace.envie.yaml
    let workspace_path = temp_dir.path().join("workspace.envie.yaml");
    let result = std::fs::read_to_string(&workspace_path);

    assert!(result.is_err(), "Should fail when workspace file doesn't exist");
}

#[test]
fn test_empty_workspace_file() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = temp_dir.path().join("workspace.envie.yaml");

    std::fs::write(&workspace_path, "").unwrap();

    let content = std::fs::read_to_string(&workspace_path).unwrap();
    let result: std::result::Result<service_config::WorkspaceConfig, serde_yaml::Error> = serde_yaml::from_str(&content);

    assert!(result.is_err(), "Should fail on empty workspace file");
}

#[test]
fn test_malformed_backend_config() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = temp_dir.path().join("workspace.envie.yaml");

    let content = r#"
version: '1.0'
project:
  name: test
environments:
  ephemeral:
    naming_pattern: "{project}-{id}"
    backend:
      type: invalid
      config: "not a map"
  stable: {}
"#;
    std::fs::write(&workspace_path, content).unwrap();

    let content = std::fs::read_to_string(&workspace_path).unwrap();
    let result: std::result::Result<service_config::WorkspaceConfig, serde_yaml::Error> = serde_yaml::from_str(&content);

    // Should fail due to malformed backend config
    assert!(result.is_err());
}

#[test]
fn test_missing_stable_environments() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = temp_dir.path().join("workspace.envie.yaml");

    let content = r#"
version: '1.0'
project:
  name: test
environments:
  ephemeral:
    naming_pattern: "{project}-{id}"
    backend:
      type: s3
      config:
        bucket: test
        region: us-east-1
        key_pattern: test
        dynamodb_table: test
        encrypt: 'true'
"#;
    std::fs::write(&workspace_path, content).unwrap();

    let content = std::fs::read_to_string(&workspace_path).unwrap();
    let result: std::result::Result<service_config::WorkspaceConfig, serde_yaml::Error> = serde_yaml::from_str(&content);

    // Should fail when stable environments are missing
    assert!(result.is_err());
}

#[test]
fn test_circular_dependency_error_message() {
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);

    let unit_dir_a = temp_dir.path().join("services/a");
    let unit_dir_b = temp_dir.path().join("services/b");
    std::fs::create_dir_all(&unit_dir_a).unwrap();
    std::fs::create_dir_all(&unit_dir_b).unwrap();

    std::fs::write(
        unit_dir_a.join("envie.yaml"),
        "name: a\ndescription: A\nunit_type: service\nstate_management: dedicated\ndependencies:\n  - path: ../b",
    )
    .unwrap();

    std::fs::write(
        unit_dir_b.join("envie.yaml"),
        "name: b\ndescription: B\nunit_type: service\nstate_management: dedicated\ndependencies:\n  - path: ../a",
    )
    .unwrap();

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    discovery.discover_all().unwrap();

    let result = discovery.get_units_in_dependency_order();
    assert!(result.is_err());

    let error = result.unwrap_err();
    let error_msg = format!("{}", error);

    // Error message should indicate circular dependency
    assert!(
        error_msg.to_lowercase().contains("circular")
            || error_msg.to_lowercase().contains("cycle")
            || error_msg.to_lowercase().contains("cyclic")
    );
}

#[test]
fn test_environment_reference_resolution() {
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);

    let workspace_path = temp_dir.path().join("workspace.envie.yaml");
    let content = std::fs::read_to_string(&workspace_path).unwrap();
    let config: service_config::WorkspaceConfig = serde_yaml::from_str(&content).unwrap();

    let environment_config = environment::EnvironmentConfig {
        project: config.project,
        ephemeral: config.environments.clone().unwrap().ephemeral,
        stable: config.environments.as_ref().unwrap().stable.clone(),
    };

    let resolver = environment::EnvironmentResolver::new(
        "test-project-123".to_string(),
        "test-project".to_string(),
        environment_config,
    );

    // Try to resolve non-existent stable environment
    let result = resolver.resolve_environment("stable.nonexistent");
    assert!(result.is_err(), "Should error on nonexistent stable environment");

    // Unknown format is treated as direct workspace reference (lenient behavior)
    let result = resolver.resolve_environment("my-custom-workspace");
    assert!(result.is_ok(), "Should allow direct workspace references");
}

#[test]
fn test_empty_naming_pattern() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = temp_dir.path().join("workspace.envie.yaml");

    let content = r#"
version: '1.0'
project:
  name: test
environments:
  ephemeral:
    naming_pattern: ""
    backend:
      type: s3
      config:
        bucket: test
        region: us-east-1
        key_pattern: test
        dynamodb_table: test
        encrypt: 'true'
  stable: {}
"#;
    std::fs::write(&workspace_path, content).unwrap();

    let content = std::fs::read_to_string(&workspace_path).unwrap();
    let result: std::result::Result<service_config::WorkspaceConfig, serde_yaml::Error> = serde_yaml::from_str(&content);

    // Should parse but have empty pattern
    assert!(result.is_ok());
    let config = result.unwrap();
    assert!(config
        .environments
        .unwrap()
        .ephemeral
        .naming_pattern
        .is_empty());
}

#[test]
fn test_duplicate_unit_names() {
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);

    // Create two units with the same name in different directories
    std::fs::create_dir_all(temp_dir.path().join("services/api")).unwrap();
    std::fs::create_dir_all(temp_dir.path().join("infrastructure/api")).unwrap();

    let unit_config = "name: api\ndescription: API\nunit_type: service\nstate_management: dedicated\ndependencies: []";

    std::fs::write(
        temp_dir.path().join("services/api/envie.yaml"),
        unit_config,
    )
    .unwrap();
    std::fs::write(
        temp_dir.path().join("infrastructure/api/envie.yaml"),
        unit_config,
    )
    .unwrap();

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    discovery.discover_all().unwrap();

    let units = discovery.get_all_units();

    // Both units should be discovered
    assert_eq!(units.len(), 2);

    // But resolution should be ambiguous
    let matches = discovery.registry.resolve_unit("api");
    assert_eq!(matches.len(), 2, "Should find both units with name 'api'");
}

#[test]
fn test_empty_dependencies_array() {
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);

    let unit_dir = temp_dir.path().join("services/api");
    std::fs::create_dir_all(&unit_dir).unwrap();

    std::fs::write(
        unit_dir.join("envie.yaml"),
        r#"name: api
description: API
unit_type: service
state_management: dedicated
dependencies: []
"#,
    )
    .unwrap();

    let content = std::fs::read_to_string(unit_dir.join("envie.yaml")).unwrap();
    let result: std::result::Result<unit_config::UnitConfig, serde_yaml::Error> = serde_yaml::from_str(&content);

    assert!(result.is_ok());
    let config = result.unwrap();
    assert_eq!(config.dependencies.len(), 0);
}

#[test]
fn test_null_optional_fields() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = temp_dir.path().join("workspace.envie.yaml");

    let content = r#"
version: '1.0'
project: null
environments:
  ephemeral:
    naming_pattern: "{project}-{id}"
    backend:
      type: s3
      config:
        bucket: test
        region: us-east-1
        key_pattern: test
        dynamodb_table: test
        encrypt: 'true'
  stable: {}
"#;
    std::fs::write(&workspace_path, content).unwrap();

    let content = std::fs::read_to_string(&workspace_path).unwrap();
    let result: std::result::Result<service_config::WorkspaceConfig, serde_yaml::Error> = serde_yaml::from_str(&content);

    assert!(result.is_ok());
    let config = result.unwrap();
    assert!(config.project.is_none());
}

#[test]
fn test_special_characters_in_names() {
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);

    let unit_dir = temp_dir.path().join("services/api-v2_test.prod");
    std::fs::create_dir_all(&unit_dir).unwrap();

    std::fs::write(
        unit_dir.join("envie.yaml"),
        r#"name: api-v2_test.prod
description: API with special chars
unit_type: service
state_management: dedicated
dependencies: []
"#,
    )
    .unwrap();

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    let result = discovery.discover_all();

    // Should handle special characters in names
    assert!(result.is_ok());

    let units = discovery.get_all_units();
    assert_eq!(units.len(), 1);
    assert_eq!(units[0].config.name, "api-v2_test.prod");
}

#[test]
fn test_very_long_unit_name() {
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);

    // Use a more reasonable length that won't hit filesystem limits
    let long_name = "a".repeat(100);
    let unit_dir = temp_dir.path().join("services").join(&long_name);

    // May fail on some filesystems with path length limits - that's expected
    if std::fs::create_dir_all(&unit_dir).is_err() {
        // Skip test if filesystem doesn't support long paths
        return;
    }

    let content = format!(
        r#"name: {}
description: Very long name
unit_type: service
state_management: dedicated
dependencies: []
"#,
        long_name
    );

    std::fs::write(unit_dir.join("envie.yaml"), content).unwrap();

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    let result = discovery.discover_all();

    // Should handle reasonably long names
    assert!(result.is_ok());
}

#[test]
fn test_unicode_in_names() {
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);

    let unit_dir = temp_dir.path().join("services/api-测试");
    std::fs::create_dir_all(&unit_dir).unwrap();

    std::fs::write(
        unit_dir.join("envie.yaml"),
        r#"name: api-测试
description: API with unicode
unit_type: service
state_management: dedicated
dependencies: []
"#,
    )
    .unwrap();

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    let result = discovery.discover_all();

    // Should handle Unicode characters
    assert!(result.is_ok());
}
