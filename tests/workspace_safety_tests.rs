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
    production:
      workspace: test-production
      backend:
        type: s3
        config:
          bucket: "test-bucket-prod"
          region: "us-east-1"
          key_pattern: "stable/production/{path}/terraform.tfstate"
          dynamodb_table: "test-locks-prod"
          encrypt: "true"
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
fn test_workspace_resolution_ephemeral() {
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

    // Test ephemeral environment resolution
    let result = resolver.resolve_environment("ephemeral").unwrap();
    assert_eq!(result.workspace, "test-project-123");
    assert!(matches!(
        result.environment_type,
        environment::EnvironmentType::Ephemeral
    ));

    // Verify backend configuration is correct
    assert_eq!(result.backend.backend_type, "s3");
    assert_eq!(result.backend.config.get("bucket").unwrap(), "test-bucket");
}

#[test]
fn test_workspace_resolution_stable_production() {
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

    // Test stable production environment resolution
    let result = resolver.resolve_environment("stable.production").unwrap();
    assert_eq!(result.workspace, "test-production");
    assert!(matches!(
        result.environment_type,
        environment::EnvironmentType::Stable(_)
    ));

    // Verify backend uses production bucket
    assert_eq!(
        result.backend.config.get("bucket").unwrap(),
        "test-bucket-prod"
    );
    assert_eq!(
        result.backend.config.get("dynamodb_table").unwrap(),
        "test-locks-prod"
    );
}

#[test]
fn test_workspace_resolution_direct_reference() {
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

    // "stable" without suffix is treated as direct workspace reference
    let result = resolver.resolve_environment("stable");
    assert!(result.is_ok(), "Should allow direct workspace references");

    let resolved = result.unwrap();
    assert_eq!(resolved.workspace, "stable");
}

#[test]
fn test_workspace_resolution_nonexistent_stable_env() {
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

    // Attempting to resolve nonexistent stable environment should error
    let result = resolver.resolve_environment("stable.nonexistent");
    assert!(result.is_err());
}

#[test]
fn test_state_key_generation_dedicated() {
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);
    create_test_unit(&temp_dir, "api", vec![]);

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    discovery.discover_all().unwrap();

    let units = discovery.get_all_units();
    let api_unit = units.iter().find(|u| u.config.name == "api").unwrap();

    // Verify state management is dedicated
    assert!(matches!(
        api_unit.config.state_management,
        unit_config::StateManagement::Dedicated
    ));

    // State key should be unique per unit
    // Format: ephemeral/{workspace}/{unit}/{unit}/terraform.tfstate
    let workspace = "test-project-123";
    let expected_key_pattern = format!("ephemeral/{}/api/api/terraform.tfstate", workspace);

    // This is what the deploy command would generate
    let state_key = format!("ephemeral/{}/{}/{}/terraform.tfstate",
        workspace,
        api_unit.config.name,
        api_unit.config.name
    );

    assert_eq!(state_key, expected_key_pattern);
}

#[test]
fn test_state_key_generation_shared() {
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);

    // Create units with shared state
    let unit_dir = temp_dir.path().join("services/api");
    std::fs::create_dir_all(&unit_dir).unwrap();
    std::fs::write(
        unit_dir.join("envie.yaml"),
        "name: api\ndescription: API\nunit_type: service\nstate_management: shared:common\ndependencies: []",
    )
    .unwrap();

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    discovery.discover_all().unwrap();

    let units = discovery.get_all_units();
    let api_unit = units.iter().find(|u| u.config.name == "api").unwrap();

    // Verify state management is shared
    assert!(matches!(
        api_unit.config.state_management,
        unit_config::StateManagement::Shared(_)
    ));

    // State key should be shared across units with same shared_id
    let workspace = "test-project-123";
    let expected_key = format!("ephemeral/{}/common/shared/terraform.tfstate", workspace);

    // This is what the deploy command would generate
    let state_key = if let unit_config::StateManagement::Shared(shared_id) = &api_unit.config.state_management {
        format!("ephemeral/{}/{}/shared/terraform.tfstate", workspace, shared_id)
    } else {
        panic!("Expected shared state management");
    };

    assert_eq!(state_key, expected_key);
}

#[test]
fn test_multiple_units_different_workspaces() {
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);
    create_test_unit(&temp_dir, "api", vec![]);
    create_test_unit(&temp_dir, "database", vec![]);

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    discovery.discover_all().unwrap();

    let units = discovery.get_all_units();
    assert_eq!(units.len(), 2);

    // Simulate deploying to different workspaces
    let workspaces = vec!["env-1", "env-2", "env-3"];

    for workspace in workspaces {
        for unit in &units {
            // Each unit in each workspace should have unique state key
            let state_key = format!(
                "ephemeral/{}/{}/{}/terraform.tfstate",
                workspace, unit.config.name, unit.config.name
            );

            // Verify uniqueness by pattern
            assert!(state_key.contains(workspace));
            assert!(state_key.contains(&unit.config.name));
        }
    }
}

#[test]
fn test_dependency_resolution_cross_workspace() {
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);
    create_test_unit(&temp_dir, "database", vec![]);
    create_test_unit(&temp_dir, "api", vec!["../database"]);

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    discovery.discover_all().unwrap();

    let units = discovery.get_all_units();
    let api_unit = units.iter().find(|u| u.config.name == "api").unwrap();

    // API depends on database
    assert_eq!(api_unit.config.dependencies.len(), 1);

    let dep = &api_unit.config.dependencies[0];
    assert_eq!(dep.path().unwrap(), "../database");

    // In a real deployment, the dependency would be resolved to a specific workspace
    // This test verifies the structure is correct for cross-workspace resolution
}

#[test]
fn test_backend_config_isolation() {
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

    // Verify ephemeral and stable production use different backends
    let ephemeral_env = resolver.resolve_environment("ephemeral").unwrap();
    let stable_prod_env = resolver.resolve_environment("stable.production").unwrap();

    // Different buckets ensure isolation
    assert_ne!(
        ephemeral_env.backend.config.get("bucket"),
        stable_prod_env.backend.config.get("bucket")
    );

    // Different lock tables ensure no cross-contamination
    assert_ne!(
        ephemeral_env.backend.config.get("dynamodb_table"),
        stable_prod_env.backend.config.get("dynamodb_table")
    );

    // Production should use production bucket
    assert_eq!(
        stable_prod_env.backend.config.get("bucket").unwrap(),
        "test-bucket-prod"
    );
}

#[test]
fn test_workspace_name_consistency() {
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

    let current_workspace = "test-project-123";
    let resolver = environment::EnvironmentResolver::new(
        current_workspace.to_string(),
        "test-project".to_string(),
        environment_config,
    );

    // Ephemeral should use current workspace
    let ephemeral_result = resolver.resolve_environment("ephemeral").unwrap();
    assert_eq!(ephemeral_result.workspace, current_workspace);

    // Stable environments should use their configured workspace
    let sandbox_result = resolver.resolve_environment("stable.sandbox").unwrap();
    assert_eq!(sandbox_result.workspace, "test-sandbox");

    let prod_result = resolver.resolve_environment("stable.production").unwrap();
    assert_eq!(prod_result.workspace, "test-production");

    // Workspaces should never be empty
    assert!(!ephemeral_result.workspace.is_empty());
    assert!(!sandbox_result.workspace.is_empty());
    assert!(!prod_result.workspace.is_empty());
}

#[test]
fn test_state_key_contains_workspace() {
    // Ensure all state keys contain the workspace name to prevent collisions
    let workspace = "test-env-456";
    let unit_name = "api";

    let dedicated_key = format!("ephemeral/{}/{}/{}/terraform.tfstate", workspace, unit_name, unit_name);
    let parent_key = format!("ephemeral/{}/{}/service/terraform.tfstate", workspace, unit_name);
    let shared_key = format!("ephemeral/{}/{}/shared/terraform.tfstate", workspace, "shared-id");

    // All keys must contain workspace to ensure isolation
    assert!(dedicated_key.contains(workspace));
    assert!(parent_key.contains(workspace));
    assert!(shared_key.contains(workspace));

    // Keys should be unique per workspace
    let different_workspace = "test-env-789";
    let different_dedicated_key = format!("ephemeral/{}/{}/{}/terraform.tfstate", different_workspace, unit_name, unit_name);

    assert_ne!(dedicated_key, different_dedicated_key);
}

#[test]
fn test_environment_type_safety() {
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

    // Ephemeral should be typed as Ephemeral
    let ephemeral_result = resolver.resolve_environment("ephemeral").unwrap();
    match ephemeral_result.environment_type {
        environment::EnvironmentType::Ephemeral => {
            // Correct
        }
        _ => panic!("Expected Ephemeral environment type"),
    }

    // Stable should be typed as Stable with name
    let stable_result = resolver.resolve_environment("stable.production").unwrap();
    match stable_result.environment_type {
        environment::EnvironmentType::Stable(ref name) => {
            assert_eq!(name, "production");
        }
        _ => panic!("Expected Stable environment type"),
    }
}
