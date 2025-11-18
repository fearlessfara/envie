# Testing Guide

## Overview

Envie has comprehensive test coverage including unit tests, integration tests, and CLI tests. The test infrastructure uses mocking to avoid requiring actual Terraform or AWS deployments.

## Test Structure

### Unit Tests (`cargo test --lib`)
Located in `src/` alongside the source code. These test individual modules and functions in isolation.

**Current Status**: 34 passing, 2 pre-existing failures in legacy code (unrelated to core functionality)

### Integration Tests (`cargo test --test integration_tests`)
Located in `tests/integration_tests.rs`. These test complete workflows:

- **Unit Discovery**: Tests finding and loading unit configurations
- **Dependency Resolution**: Tests resolving dependencies between units
- **Circular Dependency Detection**: Tests catching circular dependencies
- **Configuration Parsing**: Tests workspace and unit config parsing
- **Disambiguation**: Tests fuzzy matching for unit names
- **Environment Validation**: Tests environment configuration validation

**Current Status**: 8 passing

### CLI Tests (`cargo test --test cli_tests`)
Located in `tests/cli_tests.rs`. These test the CLI interface:

- **Help Output**: Tests rich help messages
- **Command Validation**: Tests argument validation
- **Error Messages**: Tests user-friendly error messages
- **Init Command**: Tests project initialization
- **Doctor Command**: Tests health checks
- **Unit Resolution**: Tests unit name resolution with fuzzy matching

**Current Status**: 14 passing

## Running Tests

```bash
# Run all tests
cargo test

# Run only unit tests
cargo test --lib

# Run only integration tests
cargo test --test integration_tests

# Run only CLI tests
cargo test --test cli_tests

# Run specific test
cargo test test_unit_discovery

# Run tests with output
cargo test -- --nocapture

# Run tests with detailed output
cargo test --verbose
```

## Terraform Mocking Infrastructure

To enable testing without actual Terraform/AWS deployments, we've implemented a trait-based abstraction layer.

### TerraformExecutor Trait

Located in `src/common/terraform_executor.rs`:

```rust
#[async_trait]
pub trait TerraformExecutor: Send + Sync {
    async fn init(&self, working_dir: &Path, upgrade: bool) -> Result<String>;
    async fn plan(&self, working_dir: &Path, vars: &[(&str, &str)]) -> Result<String>;
    async fn apply(&self, working_dir: &Path, vars: &[(&str, &str)]) -> Result<String>;
    async fn destroy(&self, working_dir: &Path, vars: &[(&str, &str)]) -> Result<String>;
    async fn output(&self, working_dir: &Path) -> Result<HashMap<String, serde_json::Value>>;
    async fn workspace_list(&self, working_dir: &Path) -> Result<Vec<String>>;
    async fn workspace_select(&self, working_dir: &Path, workspace: &str) -> Result<String>;
    async fn workspace_new(&self, working_dir: &Path, workspace: &str) -> Result<String>;
    async fn workspace_show(&self, working_dir: &Path) -> Result<String>;
    async fn validate(&self, working_dir: &Path) -> Result<String>;
    async fn version(&self) -> Result<String>;
}
```

### Implementations

1. **RealTerraformExecutor**: Executes actual Terraform commands (for production)
2. **MockTerraformExecutor**: Mock implementation for testing that:
   - Tracks all method calls
   - Returns configurable results
   - Can simulate success or failure scenarios
   - Doesn't require Terraform installation

### Using MockTerraformExecutor

```rust
use envie::common::terraform_executor::MockTerraformExecutor;

let mock = MockTerraformExecutor::new()
    .with_success(true)
    .with_init_result("Terraform initialized")
    .with_plan_result("Plan: 3 to add, 0 to change, 0 to destroy")
    .with_workspaces(vec!["default".to_string(), "dev".to_string()]);

// Use mock in tests
let result = mock.init(Path::new("/test"), false).await;
assert!(result.is_ok());

// Verify calls were made
let init_calls = mock.init_calls.lock().unwrap();
assert_eq!(init_calls.len(), 1);
assert_eq!(init_calls[0].0, "/test");
assert_eq!(init_calls[0].1, false);
```

## Test Fixtures

Test fixtures are located in `tests/fixtures/` and provide reusable test project structures:

### Simple Project
`tests/fixtures/simple-project/` contains:
- `workspace.envie.yaml`: Minimal workspace configuration
- `services/database/`: Database service unit
- `services/api/`: API service unit with dependency on database

These fixtures can be copied into temporary directories for test isolation.

## Continuous Integration

GitHub Actions CI pipeline (`.github/workflows/ci.yml`) runs on every push and PR:

### Jobs

1. **Test** (Ubuntu, macOS):
   - Format checking (`cargo fmt`)
   - Linting (`cargo clippy`)
   - Unit tests (`cargo test --lib`)
   - Integration tests (`cargo test --test '*'`)
   - Doc tests (`cargo test --doc`)
   - Release build

2. **Coverage**:
   - Generates code coverage with `cargo-tarpaulin`
   - Uploads to Codecov

3. **Security Audit**:
   - Runs `cargo audit` to check for known vulnerabilities

4. **Build** (Ubuntu, macOS, Windows):
   - Builds release binaries
   - Uploads artifacts

## Writing New Tests

### Integration Test Example

```rust
#[test]
fn test_my_feature() {
    let temp_dir = TempDir::new().unwrap();
    create_test_workspace(&temp_dir);
    create_test_unit(&temp_dir, "my-unit", vec![]);

    let mut discovery = unit_discovery::UnitDiscovery::new(
        temp_dir.path().to_path_buf()
    );
    discovery.discover_all().unwrap();

    let units = discovery.get_all_units();
    assert_eq!(units.len(), 1);
}
```

### CLI Test Example

```rust
#[test]
fn test_my_command() {
    let mut cmd = Command::cargo_bin("envie").unwrap();
    cmd.arg("my-command").arg("--option");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Expected output"));
}
```

## Best Practices

1. **Isolation**: Use `TempDir` for filesystem tests
2. **Cleanup**: `TempDir` automatically cleans up on drop
3. **Assertions**: Use descriptive assertion messages
4. **Mocking**: Use `MockTerraformExecutor` to avoid external dependencies
5. **Coverage**: Aim for high test coverage on critical paths
6. **Fast Tests**: Keep tests fast by avoiding real I/O when possible
7. **Clear Names**: Use descriptive test function names

## Debugging Tests

```bash
# Run with backtrace
RUST_BACKTRACE=1 cargo test test_name

# Run with full backtrace
RUST_BACKTRACE=full cargo test test_name

# Run single test with output
cargo test test_name -- --nocapture --test-threads=1
```

## Test Coverage Goals

Given the criticality of this tool (managing infrastructure deployments), we aim for:

- **Core Logic**: >90% coverage
- **CLI Commands**: >80% coverage
- **Error Handling**: 100% coverage of error paths
- **Integration Paths**: All major workflows tested

## Future Improvements

- [ ] Add property-based testing with `proptest`
- [ ] Add benchmark tests for performance regression detection
- [ ] Expand mock executor to simulate more Terraform scenarios
- [ ] Add snapshot testing for CLI output
- [ ] Increase coverage of edge cases
