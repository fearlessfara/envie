# Envie Quickstart Guide

This guide will walk you through creating your first Envie project from scratch. By the end, you'll have deployed infrastructure to an ephemeral environment with mixed stable/ephemeral dependencies.

**Time to complete**: ~15 minutes

## Prerequisites

- Envie installed (`cargo build --release`)
- AWS credentials configured
- Terraform installed (v1.0+)
- Git (optional, for branch-based workflows)

## Step 1: Create a New Project

Let's create a new project called "myapp":

```bash
# Create project directory
mkdir myapp
cd myapp

# Initialize Envie project
envie init --name myapp --description "My application infrastructure"
```

**What this does**:
- Creates `workspace.envie.yaml` (global environment configuration)
- Creates example service structure in `services/`
- Creates `.gitignore` with Envie patterns
- Creates example units with `envie.yaml` files

**Output**:
```
✅ Envie project initialized successfully!

📁 Project structure created:
  ├── workspace.envie.yaml     # Global project configuration
  ├── services/                # Service directory
  │   ├── networking/          # Example networking service
  │   ├── database/            # Example database service
  │   └── api/                 # Example API service
  └── README.md                # Project documentation

🚀 Next steps:
  1. Review and customize workspace.envie.yaml and unit envie.yaml files
  2. Add your services to the services/ directory
  3. Run 'envie deploy --unit <name> --env <id>' to deploy
```

## Step 2: Configure Environments

Open `workspace.envie.yaml` and configure your environments:

```yaml
version: '1.0'

project:
  name: myapp
  description: My application infrastructure

environments:
  # Ephemeral environments (one pattern for all temporary envs)
  ephemeral:
    naming_pattern: "{project}-{id}"
    backend:
      type: s3
      config:
        bucket: "myapp-terraform-state"
        region: "us-east-1"
        key_pattern: "ephemeral/{workspace}/{path}/terraform.tfstate"
        dynamodb_table: "myapp-terraform-locks"
        encrypt: "true"

  # Stable environments (explicitly defined)
  stable:
    sandbox:
      workspace: myapp-sandbox
      description: Sandbox environment with test data
      backend:
        type: s3
        config:
          bucket: "myapp-terraform-state"
          region: "us-east-1"
          key_pattern: "stable/sandbox/{path}/terraform.tfstate"
          dynamodb_table: "myapp-terraform-locks"
          encrypt: "true"

    production:
      workspace: myapp-production
      description: Production environment
      backend:
        type: s3
        config:
          bucket: "myapp-terraform-state-prod"
          region: "us-east-1"
          key_pattern: "production/{path}/terraform.tfstate"
          dynamodb_table: "myapp-terraform-locks-prod"
          encrypt: "true"

defaults: {}
```

**Important**: Make sure the S3 buckets exist or Envie will create them on first deployment.

## Step 3: Review Generated Structure

Envie created example units for you. Let's look at the structure:

```bash
tree services/
```

```
services/
├── api/
│   ├── envie.yaml
│   └── modules/
│       ├── gateway/
│       │   └── main.tf
│       ├── lambda/
│       │   └── main.tf
│       └── step-functions/
│           └── main.tf
├── database/
│   ├── envie.yaml
│   └── modules/
│       ├── dynamodb/
│       │   └── main.tf
│       └── rds/
│           └── main.tf
└── networking/
    ├── envie.yaml
    └── modules/
        ├── security-groups/
        │   └── main.tf
        ├── subnets/
        │   └── main.tf
        └── vpc/
            └── main.tf
```

## Step 4: Understand Unit Configuration

Let's look at the API lambda unit configuration:

```bash
cat services/api/modules/lambda/envie.yaml
```

```yaml
name: lambda
description: Lambda function for API handler
unit_type: component
state_management: dedicated

depends:
  - path: ../../../database/modules/dynamodb
    environment: stable.sandbox       # Use stable sandbox DB by default
  - path: ../../../networking/modules/vpc
    environment: ephemeral             # Use ephemeral VPC
```

**Key points**:
- `name`: Unique identifier for this unit
- `unit_type`: What kind of unit this is (component, service, module, layer)
- `state_management`: How Terraform state is managed
  - `dedicated`: Each unit gets its own state file
  - `parent`: Shares state with parent unit
  - `shared`: Shares state with other units
- `depends`: What this unit depends on and which environment to use

## Step 5: Preview Deployment (Dry Run)

Before deploying, let's see what would happen:

```bash
envie deploy --unit api --env dev-123 --dry-run
```

**Output**:
```
📋 Deployment Plan (Dry Run)

Environment: dev-123
Workspace: myapp-dev-123

Dependencies:
  ✓ ../../../database/modules/dynamodb → stable.sandbox (myapp-sandbox)
  ✓ ../../../networking/modules/vpc → ephemeral (myapp-dev-123)

Deployment Order:
  1. dynamodb (Component)
     Path: services/database/modules/dynamodb
     State: Dedicated

  2. vpc (Component)
     Path: services/networking/modules/vpc
     State: Dedicated

  3. lambda (Component)
     Path: services/api/modules/lambda
     State: Dedicated

📊 Summary:
  Total units to deploy: 3
  Component: 3
```

**What this shows**:
- Lambda will be deployed to workspace `myapp-dev-123` (ephemeral)
- Lambda will reference DynamoDB from workspace `myapp-sandbox` (stable)
- Lambda will reference VPC from workspace `myapp-dev-123` (ephemeral)
- Deployment order: dynamodb → vpc → lambda (dependencies first)

## Step 6: Deploy to Ephemeral Environment

Now let's actually deploy:

```bash
envie deploy --unit api --env dev-123 --verbose
```

**What happens**:

1. **Backend Setup** (first time only):
   ```
   🏗️  Backend Infrastructure Setup
   📦 S3 Bucket to create: myapp-terraform-state
   🔒 DynamoDB Table to create: myapp-terraform-locks
   ```
   Envie will prompt you to create the backend infrastructure if it doesn't exist.

2. **Environment Resolution**:
   ```
   🔍 Resolving dependencies:
     ├─ lambda
     ├─  ../../../database/modules/dynamodb → stable.sandbox
     │     Workspace: myapp-sandbox
     │     State: s3://myapp-terraform-state/stable/sandbox/.../terraform.tfstate
     └─  ../../../networking/modules/vpc → ephemeral
           Workspace: myapp-dev-123
           State: s3://myapp-terraform-state/ephemeral/myapp-dev-123/.../terraform.tfstate
   ```

3. **Deployment**:
   ```
   🚀 Deploying 3 unit(s)...

   [1/3] Deploying: dynamodb
     📍 Path: services/database/modules/dynamodb
     🏷️  Type: Component
     💾 State: Dedicated
     🔧 Running terraform init...
     ⚡ Running terraform apply...
     ✅ Unit deployed successfully

   [2/3] Deploying: vpc
     ...

   [3/3] Deploying: lambda
     ...

   ✅ Deployment complete!
   ```

## Step 7: Verify Deployment

Check what was created:

```bash
# List all units and their workspaces
envie list
```

**Output**:
```
📋 Listing all discovered units and their workspaces...

Active workspaces by unit:

📦 dynamodb (Component)
   Path: services/database/modules/dynamodb
   Workspaces:
     • myapp-sandbox

📦 vpc (Component)
   Path: services/networking/modules/vpc
   Workspaces:
     • myapp-dev-123

📦 lambda (Component)
   Path: services/api/modules/lambda
   Workspaces:
     • myapp-dev-123
```

## Step 8: Override Environment References

Now let's deploy the same lambda but pointing to a different database:

```bash
# Deploy lambda to a new environment, but use production database
envie deploy --unit lambda --env testing-prod-data \
  -E database:stable.production \
  --dry-run
```

**Output**:
```
📋 Deployment Plan (Dry Run)

Environment: testing-prod-data
Workspace: myapp-testing-prod-data

Dependencies:
  ✓ ../../../database/modules/dynamodb → stable.production (myapp-production)
  ✓ ../../../networking/modules/vpc → ephemeral (myapp-testing-prod-data)

Deployment Order:
  1. vpc (Component)
  2. lambda (Component)

📊 Summary:
  Total units to deploy: 2
```

Notice:
- DynamoDB now points to `myapp-production` instead of `myapp-sandbox`
- The `-E database:stable.production` override worked!

## Step 9: Deploy All Units Under a Path

Deploy all compute units at once:

```bash
envie deploy --unit services/api --env dev-456
```

This deploys:
- `services/api/modules/lambda`
- `services/api/modules/gateway`
- `services/api/modules/step-functions`

All in the correct dependency order!

## Step 10: View Generated Files

Envie generated Terraform files for you. Let's look at what was created:

```bash
cd services/api/modules/lambda
ls -la *.tf
```

```
envie-backend.tf          # Backend configuration (workspace, S3)
envie-remote-state.tf     # Remote state data sources for dependencies
main.tf                   # Your Terraform code (you wrote this)
```

**envie-backend.tf**:
```hcl
# Backend configuration generated by Envie
# State management: Dedicated
# State key: ephemeral/myapp-dev-123/services/api/modules/lambda/terraform.tfstate

terraform {
  backend "s3" {
    bucket = "myapp-terraform-state"
    region = "us-east-1"
    key = "ephemeral/myapp-dev-123/services/api/modules/lambda/terraform.tfstate"
    dynamodb_table = "myapp-terraform-locks"
    encrypt = "true"
  }
}
```

**envie-remote-state.tf**:
```hcl
# Auto-generated by Envie - DO NOT EDIT

data "terraform_remote_state" "database_dynamodb" {
  backend = "s3"
  workspace = "myapp-sandbox"

  config = {
    bucket = "myapp-terraform-state"
    region = "us-east-1"
    key = "stable/sandbox/database/dynamodb/terraform.tfstate"
    dynamodb_table = "myapp-terraform-locks"
    encrypt = "true"
  }
}

data "terraform_remote_state" "networking_vpc" {
  backend = "s3"
  workspace = "myapp-dev-123"

  config = {
    bucket = "myapp-terraform-state"
    region = "us-east-1"
    key = "ephemeral/myapp-dev-123/networking/vpc/terraform.tfstate"
    dynamodb_table = "myapp-terraform-locks"
    encrypt = "true"
  }
}
```

Now in your `main.tf`, you can reference these:

```hcl
resource "aws_lambda_function" "main" {
  function_name = "myapp-api-${var.environment_id}"

  vpc_config {
    subnet_ids = data.terraform_remote_state.networking_vpc.outputs.subnet_ids
    security_group_ids = [data.terraform_remote_state.networking_vpc.outputs.security_group_id]
  }

  environment {
    variables = {
      DB_TABLE = data.terraform_remote_state.database_dynamodb.outputs.table_name
    }
  }
}
```

## Step 11: Clean Up

When you're done with an ephemeral environment:

```bash
# Destroy just the resources
envie destroy --env dev-123

# Or completely delete everything (resources + backend)
envie delete --env dev-123
```

**destroy** vs **delete**:
- `destroy`: Destroys Terraform resources, keeps backend infrastructure (S3/DynamoDB)
- `delete`: Destroys resources AND deletes backend infrastructure (complete cleanup)

## Common Workflows

### Developer Feature Branch Workflow

```bash
# 1. Create feature branch
git checkout -b feature-auth

# 2. Deploy to ephemeral (use stable DB for speed)
envie deploy --unit api --env feature-auth \
  -E database:stable.sandbox \
  -E networking:stable.sandbox

# 3. Make code changes, redeploy
# ... edit main.tf ...
envie deploy --unit api --env feature-auth

# 4. When done, clean up
envie delete --env feature-auth
```

### Full Isolation Testing

```bash
# Deploy everything to isolated environment
envie deploy --unit services --env integration-test

# Run tests...

# Clean up
envie delete --env integration-test
```

### Testing Against Production Data

```bash
# Deploy your code to ephemeral, but read from production DB
envie deploy --unit api --env test-with-prod \
  -E database:stable.production \
  -E networking:stable.sandbox

# Your API code runs in ephemeral environment
# But reads from production database (ensure read-only!)
```

## Tips & Tricks

### 1. Use Dry Run Before Deploying
```bash
envie deploy --unit api --env my-test --dry-run
```
Always preview what will be deployed!

### 2. Use Verbose Mode for Debugging
```bash
envie deploy --unit api --env my-test --verbose
```
Shows detailed environment resolution and state file paths.

### 3. Deploy from Unit Directory
```bash
cd services/api/modules/lambda
envie deploy --env dev-123
```
Envie auto-detects you're in a unit directory.

### 4. Check What's Deployed
```bash
envie list
```
See all units and their active workspaces.

### 5. View Unit Details
```bash
envie show --unit lambda
```
See dependencies, state management, and configuration.

## Next Steps

Now that you've completed the quickstart:

1. **Customize the units**: Replace example Terraform code with your actual infrastructure
2. **Add more stable environments**: Define staging, production, etc. in `workspace.envie.yaml`
3. **Create CI/CD integration**: Use Envie in your pipeline for automated deployments
4. **Read advanced docs**:
   - [ENVIRONMENT_OVERRIDES.md](ENVIRONMENT_OVERRIDES.md) - Deep dive into environment overrides
   - [STATE_MANAGEMENT_UX_ANALYSIS.md](STATE_MANAGEMENT_UX_ANALYSIS.md) - State management details

## Troubleshooting

### Backend bucket doesn't exist
```
Error: Failed to create backend infrastructure
```
**Solution**: Make sure you have AWS credentials configured and permissions to create S3 buckets.

### Invalid environment reference
```
Error: Invalid stable environment 'stable.sanbox' in unit 'lambda'
```
**Solution**: Typo in environment name. Check `workspace.envie.yaml` for available stable environments.

### Workspace already exists
Just select the existing workspace - Envie handles this automatically.

### State file conflicts
If you get state locking errors, check if another deployment is running.

## Getting Help

- Check verbose output: `envie deploy --verbose`
- Use dry-run: `envie deploy --dry-run`
- Check [GitHub Issues](https://github.com/your-org/envie/issues)

---

**Congratulations!** You've successfully deployed infrastructure with Envie. 🎉
