# CLI Commands Reference

Complete reference for all Envie commands.

> 💡 **Tip**: Run `envie <command> --help` for detailed help and examples for any command.

## Table of Contents

- [envie init](#envie-init) - Initialize new project
- [envie deploy](#envie-deploy) - Deploy units
- [envie plan](#envie-plan) - Preview deployment
- [envie destroy](#envie-destroy) - Destroy resources
- [envie delete](#envie-delete) - Complete cleanup
- [envie list](#envie-list) - List units
- [envie show](#envie-show) - Show unit details
- [envie output](#envie-output) - Get Terraform outputs
- [envie clean](#envie-clean) - Clean Terraform cache
- [envie generate](#envie-generate) - Generate .env files
- [envie doctor](#envie-doctor) - Health checks
- [envie env](#envie-env) - Manage environments

---

## envie init

Initialize a new Envie project with configuration scaffolding.

### Usage

```bash
envie init [OPTIONS]
```

### Options

| Flag | Description |
|------|-------------|
| `--name <NAME>` | Project name (will prompt if not provided) |
| `--description <DESC>` | Project description (will prompt if not provided) |
| `--no-prompt` | Skip prompts and use defaults |
| `--verbose` | Show detailed output |

### Examples

```bash
# Interactive initialization
envie init

# With name and description
envie init --name myapp --description "My infrastructure"

# Skip prompts
envie init --name myapp --no-prompt
```

### What It Creates

- `workspace.envie.yaml` - Global configuration
- `services/` directory structure
- Example units with `envie.yaml` files
- `.gitignore` with Envie patterns
- `README.md` template

---

## envie deploy

Deploy units with dependency management and Terraform orchestration.

### Usage

```bash
envie deploy --env <ENV_ID> [OPTIONS]
```

### Options

| Flag | Description |
|------|-------------|
| `--env <ID>` | **Required**. Environment ID (e.g., dev-123) |
| `-U, --unit <UNIT>` | Unit to deploy (auto-discovers if omitted) |
| `-E, --environment <OVERRIDE>` | Override dependency environment (format: `unit:environment`) |
| `-D, --dry-run` | Preview without applying changes |
| `--no-prompt` | Skip confirmation prompts |
| `--verbose` | Show detailed environment resolution |

### Examples

```bash
# Deploy single unit
envie deploy --unit api --env dev-123

# Deploy with environment overrides
envie deploy --unit api --env feature-branch \\
  -E database:stable.sandbox \\
  -E networking:stable.sandbox

# Deploy from unit directory (auto-discovery)
cd services/api
envie deploy --env my-test

# Preview deployment
envie deploy --unit api --env test --dry-run

# Deploy with verbose output
envie deploy --unit api --env dev-123 --verbose

# Deploy all units under a path
envie deploy --unit services/api --env integration-test
```

### Environment Override Format

```
-E <unit-name>:<environment>

Examples:
-E database:stable.sandbox
-E networking:stable.production
-E api:ephemeral
```

---

## envie plan

Preview deployment without making changes (alias for `deploy --dry-run`).

### Usage

```bash
envie plan --env <ENV_ID> [OPTIONS]
```

### Options

| Flag | Description |
|------|-------------|
| `--env <ID>` | **Required**. Environment ID |
| `-U, --unit <UNIT>` | Unit to preview |
| `-E, --environment <OVERRIDE>` | Override dependency environment |
| `--verbose` | Show detailed resolution |

### Examples

```bash
# Preview deployment
envie plan --unit api --env dev-123

# Preview with overrides
envie plan --unit api --env test \\
  -E database:stable.production

# Verbose preview
envie plan --unit api --env feature-branch --verbose
```

---

## envie destroy

Destroy resources in an environment (keeps backend infrastructure).

### Usage

```bash
envie destroy --env <ENV_ID> [OPTIONS]
```

### Options

| Flag | Description |
|------|-------------|
| `--env <ID>` | Environment ID to destroy |
| `-U, --unit <UNIT>` | Specific unit to destroy |
| `-D, --dry-run` | Preview destruction |
| `--verbose` | Show detailed output |

### Examples

```bash
# Destroy all resources in environment
envie destroy --env dev-123

# Destroy specific unit
envie destroy --unit api --env test

# Preview destruction
envie destroy --env feature-branch --dry-run
```

### Note

This keeps the S3 state bucket and DynamoDB lock table. Use `envie delete` for complete cleanup.

---

## envie delete

Completely delete an environment including backend state infrastructure.

### Usage

```bash
envie delete --env <ENV_ID> [OPTIONS]
```

### Options

| Flag | Description |
|------|-------------|
| `--env <ID>` | **Required**. Environment ID to delete |
| `-U, --unit <UNIT>` | Specific unit to delete |
| `-D, --dry-run` | Preview deletion |
| `--no-prompt` | Skip confirmation |
| `--verbose` | Show detailed output |

### Examples

```bash
# Delete entire environment
envie delete --env dev-123

# Delete without confirmation
envie delete --env test --no-prompt

# Preview deletion
envie delete --env feature-branch --dry-run
```

### Warning

This is destructive and removes:
- All deployed resources
- Terraform state files
- Backend infrastructure (if no other environments use it)

---

## envie list

List all discovered units and their workspaces.

### Usage

```bash
envie list
```

### Example Output

```
📋 Listing all discovered units and their workspaces...

Active workspaces by unit:

📦 dynamodb (Component)
   Path: services/database/modules/dynamodb
   Workspaces:
     • myapp-sandbox
     • myapp-dev-123

📦 vpc (Component)
   Path: services/networking/modules/vpc
   Workspaces:
     • myapp-dev-123

📦 lambda (Component)
   Path: services/api/modules/lambda
   Workspaces:
     • myapp-dev-123
```

---

## envie show

Show detailed information about units and dependencies.

### Usage

```bash
envie show [OPTIONS]
```

### Options

| Flag | Description |
|------|-------------|
| `--unit <UNIT>` | Specific unit to show (shows all if omitted) |
| `--modules` | Show only sub-units/modules |
| `--dependencies` | Show only dependencies |
| `--verbose` | Show detailed output |

### Examples

```bash
# Show all units
envie show

# Show specific unit
envie show --unit api

# Show only dependencies
envie show --unit api --dependencies

# Show only modules
envie show --unit api --modules

# Verbose output
envie show --unit api --verbose
```

---

## envie output

Get Terraform outputs from deployed units.

### Usage

```bash
envie output --env <ENV_ID> [OPTIONS]
```

### Options

| Flag | Description |
|------|-------------|
| `--env <ID>` | **Required**. Environment ID |
| `-U, --unit <UNIT>` | Specific unit (gets all if omitted) |
| `-f, --file <PATH>` | Save to file |
| `--format <FORMAT>` | Output format: `json` or `table` (default: `table`) |
| `--verbose` | Show detailed output |

### Examples

```bash
# Get all outputs (table format)
envie output --env dev-123

# Get specific unit outputs
envie output --env dev-123 --unit api

# Save to JSON file
envie output --env dev-123 --format json --file outputs.json

# Table format
envie output --env dev-123 --format table
```

---

## envie clean

Clean Terraform cache and reinitialize.

### Usage

```bash
envie clean [OPTIONS]
```

### Options

| Flag | Description |
|------|-------------|
| `--unit <UNIT>` | Specific unit to clean |
| `--upgrade` | Run `terraform init -upgrade` |
| `--verbose` | Show detailed output |

### Examples

```bash
# Clean specific unit
envie clean --unit api

# Clean all units
envie clean

# Clean and upgrade providers
envie clean --unit api --upgrade
```

### Use Cases

- Switching between environments
- After updating Terraform providers
- Resolving state locking issues
- Clearing cached modules

---

## envie generate

Generate environment variables from Terraform outputs.

### Usage

```bash
envie generate [OPTIONS]
```

### Options

| Flag | Description |
|------|-------------|
| `--env-file <PATH>` | Template file (default: `.env.example`) |
| `--file <PATH>` | Use existing outputs.json file |

### Examples

```bash
# Generate from .env.example
envie generate

# Use custom template
envie generate --env-file .env.template

# Use existing outputs file
envie generate --file outputs.json
```

### Workflow

1. Create `.env.example` with placeholders
2. Run `envie output --env dev-123 --format json --file outputs.json`
3. Run `envie generate --file outputs.json`
4. Loads `.env` file with actual values

---

## envie doctor

Run health checks on your Envie project and environment.

### Usage

```bash
envie doctor [OPTIONS]
```

### Options

| Flag | Description |
|------|-------------|
| `--verbose` | Show detailed check output |

### Example

```bash
envie doctor
```

### Checks Performed

1. **Prerequisites**
   - Terraform installation
   - Git installation
   - AWS credentials

2. **Project Configuration**
   - workspace.envie.yaml exists and is valid
   - .gitignore has Envie patterns

3. **Unit Discovery**
   - Units discovered successfully
   - Dependency graph is valid
   - No circular dependencies
   - Units have descriptions

4. **AWS Resources**
   - S3 bucket configuration
   - DynamoDB table configuration
   - Stable environments configured

### Exit Codes

- `0` - All checks passed
- `1` - Some checks failed

---

## envie env

Manage ephemeral development environments.

### Subcommands

#### envie env start

Start a new ephemeral environment.

```bash
envie env start <ENV_ID>
```

#### envie env destroy

Destroy an ephemeral environment.

```bash
envie env destroy [ENV_ID]
```

#### envie env list

List all available environments.

```bash
envie env list
```

#### envie env current

Show the current active environment.

```bash
envie env current
```

---

## Global Flags

These flags work with most commands:

| Flag | Description |
|------|-------------|
| `--verbose` | Show detailed output |
| `--no-prompt` | Skip interactive prompts |
| `-h, --help` | Show help |
| `-V, --version` | Show version |

---

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Error occurred |
| `2` | Invalid arguments |

---

## Getting Help

```bash
# Main help
envie --help

# Command-specific help
envie deploy --help
envie plan --help

# Version
envie --version
```

---

**Previous**: [Troubleshooting](troubleshooting.md) | **Documentation Home**: [../README.md](../README.md)
