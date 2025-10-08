# Envie - Ephemeral Environment Manager for Terraform

Envie is a CLI tool that makes it easy to manage multiple ephemeral environments in Terraform with layered dependencies and flexible resource sharing.

## What is Envie?

Envie solves a common problem in infrastructure development: **how to efficiently manage multiple temporary environments** (per developer, per feature branch, per merge request) while **sharing expensive or stable resources** (databases, networking).

### Key Features

- 🚀 **Ephemeral Environments** - One environment per branch/developer, automatically managed
- 🔗 **Cross-Environment Dependencies** - Reference resources from any environment (even production!)
- 📦 **Flexible State Management** - Each unit can have dedicated or shared Terraform state
- 🌳 **Hierarchical Units** - Organize infrastructure into units (components, services, layers)
- 🔄 **Topological Deployment** - Automatic dependency resolution and ordering
- 💰 **Cost Efficient** - Share expensive resources, deploy only what changed
- 🎯 **Developer Friendly** - Simple CLI, clear error messages, verbose output

## Quick Example

```bash
# Initialize a new Envie project
envie init

# Deploy your feature to an ephemeral environment
envie deploy --unit api --env feature-auth

# Use stable sandbox database (save time/money)
envie deploy --unit api --env feature-auth -E database:stable.sandbox

# Clean up when done
envie delete --env feature-auth
```

## Installation

```bash
# Clone the repository
git clone https://github.com/your-org/envie.git
cd envie

# Build the project
cargo build --release

# Install (optional)
cargo install --path .
```

## Core Concepts

### 1. Environments

Envie has two classes of environments:

**Ephemeral** - Temporary, per-branch environments:
- `envie deploy --env feature-auth` → workspace `myapp-feature-auth`
- `envie deploy --env 123` → workspace `myapp-123` (for MR/PR #123)
- Defined once with a pattern, created on-demand
- Easy to create and destroy

**Stable** - Long-lived, shared environments:
- `stable.sandbox` - Shared development environment
- `stable.staging` - Pre-production testing
- `stable.production` - Production
- Explicitly defined in `workspace.envie`

### 2. Units

A **unit** is any deployable component:
- Component (e.g., VPC, Lambda, DynamoDB table)
- Module (e.g., subnet, security group)
- Service (e.g., API, database layer)
- Layer (e.g., networking, compute, data)

Each unit has an `envie.yaml` file describing:
- What it is (`name`, `description`, `unit_type`)
- What it depends on (`depends`)
- How state is managed (`state_management`)

### 3. Cross-Environment References

Any unit in any environment can reference any other unit in any other environment:

```yaml
# infrastructure/compute/lambda/envie.yaml
name: lambda
depends:
  - path: ../../database/dynamodb
    environment: stable.sandbox      # Use stable sandbox DB
  - path: ../../networking/vpc
    environment: ephemeral            # Use ephemeral VPC
```

## Project Structure

```
myproject/
├── workspace.envie              # Global environment configuration
├── infrastructure/
│   ├── networking/
│   │   └── vpc/
│   │       ├── envie.yaml      # Unit configuration
│   │       └── main.tf         # Your Terraform code
│   ├── database/
│   │   └── dynamodb/
│   │       ├── envie.yaml
│   │       └── main.tf
│   └── compute/
│       └── lambda/
│           ├── envie.yaml
│           └── main.tf
└── .gitignore
```

## Getting Started

See [QUICKSTART.md](QUICKSTART.md) for a complete tutorial that walks through:
1. Creating a new project
2. Defining environments
3. Creating infrastructure units
4. Deploying to ephemeral environments
5. Using stable dependencies
6. Cleaning up

## Documentation

- **[QUICKSTART.md](QUICKSTART.md)** - Step-by-step tutorial for new users
- **[ENVIRONMENT_OVERRIDES.md](ENVIRONMENT_OVERRIDES.md)** - Guide to environment overrides and mixed deployments
- **[STATE_MANAGEMENT_UX_ANALYSIS.md](STATE_MANAGEMENT_UX_ANALYSIS.md)** - Deep dive into state management
- **[UX_IMPROVEMENTS_IMPLEMENTED.md](UX_IMPROVEMENTS_IMPLEMENTED.md)** - Recent UX improvements

## Common Commands

```bash
# Initialize a new project
envie init

# List all units
envie list

# Deploy a unit (and its dependencies)
envie deploy --unit <unit-name> --env <environment-id>

# Deploy with environment overrides
envie deploy --unit api --env dev-123 -E database:stable.sandbox

# Deploy all units under a path
envie deploy --unit infrastructure/compute --env dev-123

# Preview deployment (dry-run)
envie deploy --unit api --env dev-123 --dry-run

# Verbose output (show environment resolution)
envie deploy --unit api --env dev-123 --verbose

# Destroy resources in an environment
envie destroy --env dev-123

# Complete deletion (resources + backend)
envie delete --env dev-123

# Show unit details
envie show --unit api
```

## Real-World Scenarios

### Scenario 1: Frontend Developer (API Changes Only)

You're working on API changes but don't want to redeploy the entire stack:

```bash
# Deploy only API to ephemeral, use stable sandbox for everything else
envie deploy --unit api --env feature-auth \
  -E database:stable.sandbox \
  -E networking:stable.sandbox
```

**Result**: Fast deployment, uses existing shared infrastructure, your API is isolated.

### Scenario 2: Full Stack Testing

You need complete isolation for integration testing:

```bash
# Deploy everything to your own ephemeral environment
envie deploy --unit infrastructure --env e2e-test
```

**Result**: Complete isolated environment, all services use ephemeral versions.

### Scenario 3: Testing Against Production Data

You need to test with production data (read-only):

```bash
# Deploy your code to ephemeral, but reference production database
envie deploy --unit api --env test-prod-data \
  -E database:stable.production \
  -E networking:stable.sandbox
```

**Result**: Your code in ephemeral, reads from production DB.

## How It Works

When you run `envie deploy --unit lambda --env feature-auth`:

1. **Discovery**: Envie finds all units by walking the directory tree looking for `envie.yaml` files
2. **Validation**: Validates environment references (catches typos early)
3. **Resolution**: Resolves dependencies and determines deployment order
4. **Generation**: For each unit, generates:
   - `envie-backend.tf` - Terraform backend configuration
   - `envie-remote-state.tf` - Remote state data sources for dependencies
   - `terraform.tfvars` - Environment-specific variables
5. **Deployment**: Runs `terraform init` and `terraform apply` for each unit in order

## State Management

Envie generates Terraform remote state references automatically:

```hcl
# envie-remote-state.tf (auto-generated)
data "terraform_remote_state" "database_dynamodb" {
  backend = "s3"
  workspace = "myapp-sandbox"

  config = {
    bucket = "myapp-terraform-state"
    key = "stable/sandbox/database/dynamodb/terraform.tfstate"
  }
}
```

You just reference it in your Terraform code:

```hcl
# main.tf (your code)
resource "aws_lambda_function" "main" {
  environment {
    variables = {
      DB_TABLE = data.terraform_remote_state.database_dynamodb.outputs.table_name
    }
  }
}
```

## Best Practices

1. **Use stable environments for shared resources**
   - Databases with test data
   - Expensive networking infrastructure
   - Third-party integrations

2. **Use ephemeral for active development**
   - Services being actively modified
   - Features under development
   - Experimental changes

3. **Deploy only what changed**
   - Don't redeploy the entire stack for a small API change
   - Use stable dependencies to save time and money

4. **Clean up ephemeral environments**
   - Use `envie delete --env <env-id>` when done with a feature
   - Prevents accumulation of unused resources

5. **Use dry-run before deployment**
   - Always run with `--dry-run` first to preview changes
   - Verify environment resolution is correct

## Contributing

Contributions are welcome! Please open an issue or pull request.

## License

[Add your license here]

## Support

For issues and questions:
- GitHub Issues: https://github.com/your-org/envie/issues
- Documentation: See docs/ folder
