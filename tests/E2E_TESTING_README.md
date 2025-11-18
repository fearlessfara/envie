# End-to-End Terraform Validation Tests

## Overview

The E2E tests in `e2e_terraform_validation_tests.rs` validate that Envie generates **actual, valid Terraform code** that can be used with the real Terraform CLI.

Unlike the mock-based tests, these tests:
1. Generate real Terraform `.tf` files
2. Run `terraform init -backend=false` to download providers
3. Run `terraform validate` to verify HCL syntax and semantics

This ensures our Terraform generation is production-ready, not just mock-compatible.

## Prerequisites

**Terraform CLI must be installed** to run these tests. The tests will automatically skip if Terraform is not available.

### Installing Terraform

**macOS:**
```bash
brew install terraform
```

**Linux (Ubuntu/Debian):**
```bash
wget -O- https://apt.releases.hashicorp.com/gpg | sudo gpg --dearmor -o /usr/share/keyrings/hashicorp-archive-keyring.gpg
echo "deb [signed-by=/usr/share/keyrings/hashicorp-archive-keyring.gpg] https://apt.releases.hashicorp.com $(lsb_release -cs) main" | sudo tee /etc/apt/sources.list.d/hashicorp.list
sudo apt update && sudo apt install terraform
```

**Windows:**
```powershell
choco install terraform
```

Or download from: https://www.terraform.io/downloads

## Running the Tests

```bash
# Run only E2E tests
cargo test --test e2e_terraform_validation_tests

# Run with output to see terraform commands
cargo test --test e2e_terraform_validation_tests -- --nocapture

# Run specific E2E test
cargo test --test e2e_terraform_validation_tests test_diamond_dependency_pattern

# Run all tests including E2E
cargo test
```

## Test Scenarios

### 1. Simple Unit Validation
**Test**: `test_simple_unit_terraform_validation`

Validates a single unit with no dependencies:
- Creates minimal Terraform configuration
- Runs `terraform init` to download providers
- Runs `terraform validate` to check syntax

**Purpose**: Ensures basic Terraform generation works

### 2. Unit with Variables
**Test**: `test_unit_with_variables_terraform_validation`

Validates a unit with custom variables:
- Database instance class
- Database name
- Output values

**Purpose**: Ensures variable declarations are valid

### 3. Linear Dependency Chain
**Test**: `test_linear_dependency_chain`

Tests a 3-unit chain:
```
api → database → networking
```

**Purpose**: Validates dependency ordering and references

### 4. Diamond Dependency Pattern
**Test**: `test_diamond_dependency_pattern`

Tests complex dependency graph:
```
       api
      /   \
  database cache
      \   /
    networking
```

**Purpose**: Validates complex dependency resolution

### 5. Complex Microservices Architecture
**Test**: `test_complex_microservices_architecture`

Tests realistic microservices setup:
```
api_gateway → (auth, user)
auth → database
user → (database, cache)
database → vpc
cache → vpc
```

**Purpose**: Validates real-world architecture patterns

### 6. Remote State Data Sources
**Test**: `test_unit_with_remote_state_data_sources`

Tests units that reference outputs from other units:
- Database outputs endpoint and port
- API references database outputs

**Purpose**: Ensures remote state data sources work

### 7. Multi-Provider Configuration
**Test**: `test_unit_with_provider_configuration`

Tests units with multiple AWS provider configurations:
- Primary provider (us-east-1)
- Secondary provider (us-west-2)

**Purpose**: Validates provider alias configuration

### 8. Invalid Syntax Detection
**Test**: `test_invalid_terraform_syntax_detection`

Tests that validation correctly rejects invalid HCL:
- Missing closing braces
- Syntax errors

**Purpose**: Ensures validation actually catches errors

### 9. Missing Variable Detection
**Test**: `test_missing_required_variable`

Tests that validation catches undefined variable references:
- References `var.undefined_var`

**Purpose**: Ensures semantic validation works

## How It Works

### 1. Test Setup
Each test creates a temporary directory with:
- `workspace.envie.yaml` - Envie workspace configuration
- `units/<name>/envie.yaml` - Unit configuration
- `units/<name>/main.tf` - Terraform code

### 2. Terraform Init
```bash
terraform init -backend=false
```

This downloads required providers without configuring a backend. The `-backend=false` flag allows testing without S3/remote state.

### 3. Terraform Validate
```bash
terraform validate -json
```

This checks:
- HCL syntax correctness
- Variable references are defined
- Resource configurations are valid
- Provider requirements are met

### 4. Assertion
Tests verify:
- Init succeeds (providers download)
- Validate succeeds (code is valid)
- JSON output contains `"valid": true`

## Test Output

### Successful Test
```
test test_simple_unit_terraform_validation ... ok
```

### Skipped Test (No Terraform)
```
test test_simple_unit_terraform_validation ... ok
Skipping test: terraform not available
```

### Failed Test
```
test test_simple_unit_terraform_validation ... FAILED

Validation should succeed: Err(ProcessError("Terraform validation failed:
stdout: ...
stderr: Error: Missing required argument"))
```

## CI/CD Integration

### GitHub Actions Example
```yaml
jobs:
  e2e-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Setup Terraform
        uses: hashicorp/setup-terraform@v2
        with:
          terraform_version: 1.5.0

      - name: Run E2E Tests
        run: cargo test --test e2e_terraform_validation_tests
```

### Docker Testing
```dockerfile
FROM rust:latest

# Install Terraform
RUN apt-get update && apt-get install -y wget unzip
RUN wget https://releases.hashicorp.com/terraform/1.5.0/terraform_1.5.0_linux_amd64.zip
RUN unzip terraform_1.5.0_linux_amd64.zip -d /usr/local/bin/

# Run tests
RUN cargo test --test e2e_terraform_validation_tests
```

## Benefits

### 1. Real Validation
Unlike mocks, these tests use the actual Terraform CLI, catching:
- HCL syntax errors
- Invalid resource configurations
- Provider requirement issues
- Variable reference errors

### 2. Provider Compatibility
Tests download real providers, ensuring compatibility with:
- AWS provider
- Other providers used in generated code

### 3. Regression Prevention
If we break Terraform generation, these tests will fail immediately.

### 4. Confidence
Passing E2E tests means the generated Terraform can be used in production.

## Debugging Failed Tests

### Check Terraform Output
```bash
cargo test --test e2e_terraform_validation_tests test_name -- --nocapture
```

### Manual Inspection
Failed tests leave temporary directories. To inspect:

1. Add `std::mem::forget(temp_dir);` before the test ends
2. Run the test
3. Check `/tmp` for the test directory
4. Manually run terraform:
```bash
cd /tmp/test-directory
terraform init -backend=false
terraform validate
```

### Common Issues

**Provider Download Fails**
- Check internet connection
- Verify Terraform can reach registry.terraform.io

**Validation Fails**
- Check HCL syntax in generated files
- Verify all variables are defined
- Check resource configurations

**Init Fails**
- Verify Terraform version compatibility
- Check provider requirements

## Future Enhancements

- [ ] Add `terraform plan` tests with mock resources
- [ ] Test state file formats
- [ ] Test workspace operations
- [ ] Add provider version pinning tests
- [ ] Test module sources
- [ ] Add performance benchmarks for large dependency graphs
