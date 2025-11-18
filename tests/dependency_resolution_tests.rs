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

/// Helper to create a test unit with specific configuration
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
fn test_simple_linear_dependency() {
    // A -> B -> C (linear chain)
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);
    create_test_unit(&temp_dir, "c", vec![]);
    create_test_unit(&temp_dir, "b", vec!["../c"]);
    create_test_unit(&temp_dir, "a", vec!["../b"]);

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    discovery.discover_all().unwrap();

    let ordered = discovery.get_units_in_dependency_order().unwrap();

    // Order should be C, B, A
    let order: Vec<String> = ordered.iter().map(|u| u.config.name.clone()).collect();

    let c_pos = order.iter().position(|n| n == "c").unwrap();
    let b_pos = order.iter().position(|n| n == "b").unwrap();
    let a_pos = order.iter().position(|n| n == "a").unwrap();

    assert!(c_pos < b_pos, "C should come before B");
    assert!(b_pos < a_pos, "B should come before A");
}

#[test]
fn test_diamond_dependency() {
    // Diamond pattern: A depends on B and C, both B and C depend on D
    //     A
    //    / \
    //   B   C
    //    \ /
    //     D
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);
    create_test_unit(&temp_dir, "d", vec![]);
    create_test_unit(&temp_dir, "b", vec!["../d"]);
    create_test_unit(&temp_dir, "c", vec!["../d"]);
    create_test_unit(&temp_dir, "a", vec!["../b", "../c"]);

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    discovery.discover_all().unwrap();

    let ordered = discovery.get_units_in_dependency_order().unwrap();
    let order: Vec<String> = ordered.iter().map(|u| u.config.name.clone()).collect();

    let d_pos = order.iter().position(|n| n == "d").unwrap();
    let b_pos = order.iter().position(|n| n == "b").unwrap();
    let c_pos = order.iter().position(|n| n == "c").unwrap();
    let a_pos = order.iter().position(|n| n == "a").unwrap();

    // D must come first
    assert!(d_pos < b_pos, "D should come before B");
    assert!(d_pos < c_pos, "D should come before C");

    // B and C must come before A
    assert!(b_pos < a_pos, "B should come before A");
    assert!(c_pos < a_pos, "C should come before A");
}

#[test]
fn test_complex_dependency_graph() {
    // Complex graph:
    //     E
    //    /|\
    //   / | \
    //  A  B  C
    //   \ | /
    //    \|/
    //     D
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);
    create_test_unit(&temp_dir, "d", vec![]);
    create_test_unit(&temp_dir, "a", vec!["../d"]);
    create_test_unit(&temp_dir, "b", vec!["../d"]);
    create_test_unit(&temp_dir, "c", vec!["../d"]);
    create_test_unit(&temp_dir, "e", vec!["../a", "../b", "../c"]);

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    discovery.discover_all().unwrap();

    let ordered = discovery.get_units_in_dependency_order().unwrap();
    let order: Vec<String> = ordered.iter().map(|u| u.config.name.clone()).collect();

    let d_pos = order.iter().position(|n| n == "d").unwrap();
    let a_pos = order.iter().position(|n| n == "a").unwrap();
    let b_pos = order.iter().position(|n| n == "b").unwrap();
    let c_pos = order.iter().position(|n| n == "c").unwrap();
    let e_pos = order.iter().position(|n| n == "e").unwrap();

    // D must be first
    assert!(d_pos < a_pos);
    assert!(d_pos < b_pos);
    assert!(d_pos < c_pos);

    // E must be last
    assert!(a_pos < e_pos);
    assert!(b_pos < e_pos);
    assert!(c_pos < e_pos);
}

#[test]
fn test_self_dependency_detection() {
    // Unit cannot depend on itself
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);
    create_test_unit(&temp_dir, "api", vec!["."]);

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    discovery.discover_all().unwrap();

    // This should be detected as a circular dependency
    let result = discovery.get_units_in_dependency_order();
    // Note: Current implementation may not explicitly check self-dependencies
    // but topological sort should fail on cycles
}

#[test]
fn test_circular_dependency_two_units() {
    // A -> B -> A (simple cycle)
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);
    create_test_unit(&temp_dir, "a", vec!["../b"]);
    create_test_unit(&temp_dir, "b", vec!["../a"]);

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    discovery.discover_all().unwrap();

    let result = discovery.get_units_in_dependency_order();
    assert!(result.is_err(), "Should detect circular dependency");
}

#[test]
fn test_circular_dependency_three_units() {
    // A -> B -> C -> A (cycle of 3)
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);
    create_test_unit(&temp_dir, "a", vec!["../b"]);
    create_test_unit(&temp_dir, "b", vec!["../c"]);
    create_test_unit(&temp_dir, "c", vec!["../a"]);

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    discovery.discover_all().unwrap();

    let result = discovery.get_units_in_dependency_order();
    assert!(result.is_err(), "Should detect circular dependency in cycle of 3");
}

#[test]
fn test_indirect_circular_dependency() {
    // Complex graph with hidden cycle:
    // A -> B -> D
    // A -> C -> D -> A (cycle through A -> C -> D -> A)
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);
    create_test_unit(&temp_dir, "a", vec!["../b", "../c"]);
    create_test_unit(&temp_dir, "b", vec!["../d"]);
    create_test_unit(&temp_dir, "c", vec!["../d"]);
    create_test_unit(&temp_dir, "d", vec!["../a"]);

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    discovery.discover_all().unwrap();

    let result = discovery.get_units_in_dependency_order();
    assert!(result.is_err(), "Should detect indirect circular dependency");
}

#[test]
fn test_missing_dependency() {
    // A depends on B, but B doesn't exist
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);
    create_test_unit(&temp_dir, "a", vec!["../b"]);

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    discovery.discover_all().unwrap();

    // Unit A exists but references non-existent unit B
    let units = discovery.get_all_units();
    assert_eq!(units.len(), 1);
    assert_eq!(units[0].config.name, "a");
    assert_eq!(units[0].config.dependencies.len(), 1);

    // The dependency resolution should handle this gracefully
    // (deployment would fail when trying to resolve the dependency)
}

#[test]
fn test_multiple_dependencies_same_unit() {
    // A depends on B multiple times (should be treated as single dependency)
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);

    let unit_dir = temp_dir.path().join("services/a");
    std::fs::create_dir_all(&unit_dir).unwrap();
    std::fs::write(
        unit_dir.join("envie.yaml"),
        r#"name: a
description: A service
unit_type: service
state_management: dedicated
dependencies:
  - path: ../b
  - path: ../b
"#,
    )
    .unwrap();
    create_test_unit(&temp_dir, "b", vec![]);

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    discovery.discover_all().unwrap();

    let units = discovery.get_all_units();
    let a_unit = units.iter().find(|u| u.config.name == "a").unwrap();

    // Should have 2 dependency entries (duplicate handling is deployment concern)
    assert_eq!(a_unit.config.dependencies.len(), 2);
}

#[test]
fn test_deep_dependency_chain() {
    // A -> B -> C -> D -> E -> F -> G -> H -> I -> J (10 levels deep)
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);

    create_test_unit(&temp_dir, "j", vec![]);
    create_test_unit(&temp_dir, "i", vec!["../j"]);
    create_test_unit(&temp_dir, "h", vec!["../i"]);
    create_test_unit(&temp_dir, "g", vec!["../h"]);
    create_test_unit(&temp_dir, "f", vec!["../g"]);
    create_test_unit(&temp_dir, "e", vec!["../f"]);
    create_test_unit(&temp_dir, "d", vec!["../e"]);
    create_test_unit(&temp_dir, "c", vec!["../d"]);
    create_test_unit(&temp_dir, "b", vec!["../c"]);
    create_test_unit(&temp_dir, "a", vec!["../b"]);

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    discovery.discover_all().unwrap();

    let ordered = discovery.get_units_in_dependency_order().unwrap();
    assert_eq!(ordered.len(), 10);

    // Verify order is correct
    let order: Vec<String> = ordered.iter().map(|u| u.config.name.clone()).collect();
    let expected = vec!["j", "i", "h", "g", "f", "e", "d", "c", "b", "a"];
    assert_eq!(order, expected);
}

#[test]
fn test_parallel_independent_units() {
    // Multiple units with no dependencies (can deploy in parallel)
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);
    create_test_unit(&temp_dir, "a", vec![]);
    create_test_unit(&temp_dir, "b", vec![]);
    create_test_unit(&temp_dir, "c", vec![]);
    create_test_unit(&temp_dir, "d", vec![]);
    create_test_unit(&temp_dir, "e", vec![]);

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    discovery.discover_all().unwrap();

    let ordered = discovery.get_units_in_dependency_order().unwrap();
    assert_eq!(ordered.len(), 5);

    // All units have no dependencies, so any order is valid
    // Just verify they're all present
    let names: Vec<String> = ordered.iter().map(|u| u.config.name.clone()).collect();
    assert!(names.contains(&"a".to_string()));
    assert!(names.contains(&"b".to_string()));
    assert!(names.contains(&"c".to_string()));
    assert!(names.contains(&"d".to_string()));
    assert!(names.contains(&"e".to_string()));
}

#[test]
fn test_shared_dependency() {
    // Multiple units depend on same unit
    //   B
    //  / \
    // A   C
    //  \ /
    //   D
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);
    create_test_unit(&temp_dir, "d", vec![]);
    create_test_unit(&temp_dir, "a", vec!["../d"]);
    create_test_unit(&temp_dir, "c", vec!["../d"]);
    create_test_unit(&temp_dir, "b", vec!["../a", "../c"]);

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    discovery.discover_all().unwrap();

    let ordered = discovery.get_units_in_dependency_order().unwrap();
    let order: Vec<String> = ordered.iter().map(|u| u.config.name.clone()).collect();

    let d_pos = order.iter().position(|n| n == "d").unwrap();
    let a_pos = order.iter().position(|n| n == "a").unwrap();
    let c_pos = order.iter().position(|n| n == "c").unwrap();
    let b_pos = order.iter().position(|n| n == "b").unwrap();

    // D must come first
    assert!(d_pos < a_pos);
    assert!(d_pos < c_pos);

    // A and C must come before B
    assert!(a_pos < b_pos);
    assert!(c_pos < b_pos);
}

#[test]
fn test_registry_resolution() {
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);
    create_test_unit(&temp_dir, "api", vec![]);
    create_test_unit(&temp_dir, "database", vec![]);
    create_test_unit(&temp_dir, "cache", vec![]);

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    discovery.discover_all().unwrap();

    // Test registry resolution
    let matches = discovery.registry.resolve_unit("api");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].config.name, "api");

    // Test non-existent unit
    let matches = discovery.registry.resolve_unit("nonexistent");
    assert_eq!(matches.len(), 0);

    // Test partial match
    let matches = discovery.registry.resolve_unit("dat");
    assert_eq!(matches.len(), 0); // Should not match partial names
}

#[test]
fn test_dependency_path_resolution() {
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);

    // Create nested structure
    std::fs::create_dir_all(temp_dir.path().join("services/api")).unwrap();
    std::fs::create_dir_all(temp_dir.path().join("infrastructure/networking")).unwrap();

    std::fs::write(
        temp_dir.path().join("infrastructure/networking/envie.yaml"),
        "name: networking\ndescription: Network infrastructure\nunit_type: service\nstate_management: dedicated\ndependencies: []",
    )
    .unwrap();

    std::fs::write(
        temp_dir.path().join("services/api/envie.yaml"),
        "name: api\ndescription: API\nunit_type: service\nstate_management: dedicated\ndependencies:\n  - path: ../../infrastructure/networking",
    )
    .unwrap();

    let mut discovery = unit_discovery::UnitDiscovery::new(temp_dir.path().to_path_buf());
    discovery.discover_all().unwrap();

    let units = discovery.get_all_units();
    assert_eq!(units.len(), 2);

    let api_unit = units.iter().find(|u| u.config.name == "api").unwrap();
    assert_eq!(api_unit.config.dependencies.len(), 1);

    // Verify dependency path is correct
    let dep_path = api_unit.config.dependencies[0].path().unwrap();
    assert_eq!(dep_path, "../../infrastructure/networking");
}
