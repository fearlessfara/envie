# Environment Overrides in Envie

## Overview

Envie allows you to deploy units with **mixed environments** - crucial for development efficiency. You can deploy your API to an ephemeral dev environment while pointing it to a stable sandbox database, avoiding expensive redeployment of unchanged infrastructure.

## How It Works

### 1. Define Dependencies with Default Environments

In your `envie.yaml`, specify which environment each dependency should use by default:

```yaml
# infrastructure/compute/lambda/envie.yaml
name: lambda
description: Lambda function
unit_type: component
state_management: dedicated

depends:
  - path: ../../networking/vpc
    environment: ephemeral          # Use ephemeral VPC by default
  - path: ../../database/dynamodb
    environment: stable.sandbox     # Use stable sandbox DB by default
```

### 2. Override Environments at Deploy Time

When deploying, you can override any dependency's environment using the `-E` flag:

```bash
# Deploy API to ephemeral env, but use sandbox database
envie deploy --unit api --env dev-123 \
  -E database:stable.sandbox \
  -E networking:ephemeral

# Deploy everything to your ephemeral environment
envie deploy --unit api --env dev-123 \
  -E database:ephemeral \
  -E networking:ephemeral
```

## Environment Types

### Ephemeral Environments
Temporary environments for development/testing:

- **`ephemeral`** - Uses the current environment being deployed
- **`ephemeral.123`** - Uses a specific ephemeral environment (e.g., MR 123)

Example:
```yaml
depends:
  - path: ../../networking/vpc
    environment: ephemeral  # Will use myapp-dev-123 if deploying to dev-123
```

### Stable Environments
Long-lived environments defined in `workspace.envie`:

- **`stable.sandbox`** - References the "sandbox" stable environment
- **`stable.production`** - References the "production" stable environment

Example:
```yaml
depends:
  - path: ../../database/dynamodb
    environment: stable.sandbox  # Always uses sandbox database
```

## Real-World Scenarios

### Scenario 1: Frontend Developer (API Changes Only)

**Problem**: Frontend dev needs to test API changes but doesn't want to redeploy the entire infrastructure.

**Solution**:
```bash
# Deploy only the API to ephemeral env, use stable sandbox for everything else
envie deploy --unit infrastructure/compute/api --env feature-auth \
  -E database:stable.sandbox \
  -E networking:stable.sandbox
```

**Result**:
- ✅ Fast deployment (only API rebuilds)
- ✅ Uses existing sandbox database (data preserved)
- ✅ Uses existing sandbox VPC (no networking changes)
- ✅ Cost-effective

### Scenario 2: Database Schema Changes

**Problem**: Need to test database migrations without affecting sandbox.

**Solution**:
```bash
# Deploy database AND API to ephemeral environment
envie deploy --unit infrastructure --env db-migration-test \
  -E database:ephemeral \
  -E networking:stable.sandbox  # Still use stable VPC
```

**Result**:
- ✅ Isolated database for testing
- ✅ API connects to ephemeral database
- ✅ Sandbox database unchanged
- ✅ Easy cleanup (just destroy the ephemeral env)

### Scenario 3: Full Stack Testing

**Problem**: Testing end-to-end changes across all services.

**Solution**:
```bash
# Deploy everything to ephemeral (no overrides needed)
envie deploy --unit infrastructure --env e2e-test
```

**Result**:
- ✅ Complete isolated environment
- ✅ All services use ephemeral versions
- ✅ Perfect for integration testing
- ✅ Clean slate every time

### Scenario 4: Path-Based Deployment with Overrides

**Problem**: Deploying all compute units but want to use stable database.

**Solution**:
```bash
# Deploy all units under infrastructure/compute
envie deploy --unit infrastructure/compute --env my-feature \
  -E database:stable.sandbox \
  -E networking:stable.sandbox
```

**Result**:
- ✅ All compute units deployed to ephemeral
- ✅ They all connect to stable sandbox dependencies
- ✅ Fast iteration on compute layer only

## Configuration in workspace.envie

Define your stable environments:

```yaml
project:
  name: myapp

ephemeral:
  naming_pattern: "{project}-{env}"
  backend:
    type: s3
    config:
      bucket: myapp-terraform-state
      region: us-east-1
      key: "ephemeral/{workspace}/terraform.tfstate"

stable:
  sandbox:
    workspace: myapp-sandbox
    description: Shared sandbox environment for development
    backend:
      type: s3
      config:
        bucket: myapp-terraform-state
        region: us-east-1
        key: "stable/sandbox/{service}/terraform.tfstate"

  production:
    workspace: myapp-prod
    description: Production environment
    backend:
      type: s3
      config:
        bucket: myapp-terraform-state-prod
        region: us-east-1
        key: "production/{service}/terraform.tfstate"
```

## Benefits

1. **Cost Savings**: Don't redeploy expensive infrastructure unnecessarily
2. **Speed**: Deploy only what changed
3. **Flexibility**: Mix and match environments per dependency
4. **Safety**: Test against stable data without modifying it
5. **Developer Experience**: Each developer can work on their part without full stack deployment

## Best Practices

1. **Use stable environments for shared resources**:
   - Databases with test data
   - Networking infrastructure
   - Third-party integrations

2. **Use ephemeral for active development**:
   - Services being actively modified
   - Features under development
   - Experimental changes

3. **Document your defaults**:
   ```yaml
   # Good: Clear intent
   depends:
     - path: ../../database/dynamodb
       environment: stable.sandbox  # Using sandbox for cost savings
   ```

4. **Override at deploy time when needed**:
   ```bash
   # Override when you need full isolation
   envie deploy --unit api --env my-test -E database:ephemeral
   ```

## Example Workflow

```bash
# Day 1: Set up your feature branch
envie deploy --unit infrastructure --env feature-123

# Day 2: Only working on API, use stable dependencies
envie deploy --unit api --env feature-123 \
  -E database:stable.sandbox \
  -E networking:stable.sandbox

# Day 3: Need to test database changes too
envie deploy --unit api --env feature-123 \
  -E database:ephemeral \
  -E networking:stable.sandbox

# Day 4: Full integration test before merge
envie deploy --unit infrastructure --env feature-123
  # (Uses defaults from envie.yaml)

# Cleanup
envie delete --env feature-123
```

This approach gives you maximum flexibility while minimizing costs and deployment time! 🚀
