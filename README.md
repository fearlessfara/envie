# Envie

A tool for managing multiple ephemeral environments in Terraform with layered dependencies and resource sharing.

## ✨ Features

- **Ephemeral Environments**: Create temporary environments for feature branches, PRs, and testing
- **Dependency Management**: Automatically resolve and deploy dependencies in correct order
- **Flexible Resource Sharing**: Mix ephemeral and stable resources (e.g., ephemeral API + stable database)
- **Auto-Generated Terraform**: Automatic backend and remote state configuration
- **Environment Overrides**: Override dependencies per-deployment with `-E` flags
- **Health Checks**: Built-in diagnostics with `envie doctor`

## 🚀 Quick Start

### 1. Install

```bash
# Build from source
cargo build --release

# Add to PATH (optional)
export PATH="$PWD/target/release:$PATH"
```

### 2. Initialize Project

```bash
# Create new project with example services
envie init --name myapp

# This creates:
#   - workspace.envie.yaml (project config)
#   - services/ (units directory)
#   - 3 example services (networking, database, api)
```

### 3. Scaffold Additional Units

```bash
# Create new units with templates
envie scaffold auth --template api
envie gen worker -t compute           # Short alias

# Or interactively choose template
envie scaffold myservice
```

**Available Templates:**
- `simple` - Basic unit with single main.tf
- `with-modules` - Multi-module unit structure
- `networking` - VPC, subnets, security groups
- `database` - DynamoDB, RDS
- `api` - Lambda + API Gateway
- `compute` - Lambda functions

### 4. Deploy to Environment

```bash
# Preview deployment (dry-run)
envie plan --unit api --env dev-123

# Deploy to ephemeral environment
envie deploy --unit api --env dev-123

# Deploy with stable dependencies
envie deploy --unit api --env feature-auth \
  -E database:stable.sandbox \
  -E networking:stable.sandbox
```

### 5. Manage Environments

```bash
# List all deployed units
envie list

# Get outputs from environment
envie output --env dev-123

# Clean up when done
envie delete --env dev-123
```

## 📚 Documentation

**New to Envie?** Start here:
- [Installation Guide](docs/getting-started/installation.md)
- [Quick Start (15 min)](docs/getting-started/quickstart.md)
- [Core Concepts](docs/getting-started/concepts.md)

**Ready to deploy?** Check out:
- [Common Workflows](docs/guides/workflows.md)
- [Environment Overrides Guide](docs/guides/environment-overrides.md)
- [CLI Commands Reference](docs/reference/commands.md)

**Need help?**
- Run `envie doctor` for health checks
- Check [Troubleshooting Guide](docs/reference/troubleshooting.md)
- View [Complete Documentation](docs/README.md)

## 💻 Example Usage

### Deploy with Environment Overrides

```bash
# Deploy API to ephemeral, but use stable database
envie deploy --unit api --env feature-auth \
  -E database:stable.sandbox \
  -E networking:stable.sandbox
```

### Preview Before Deploying

```bash
# See what will be deployed
envie plan --unit api --env test
```

### List Deployed Resources

```bash
# See all units and their workspaces
envie list
```

## 🏗️ Project Structure

```
myapp/
├── workspace.envie.yaml          # Global configuration
├── services/
│   ├── networking/
│   │   └── envie.yaml             # Unit configuration
│   ├── database/
│   │   └── envie.yaml
│   └── api/
│       ├── envie.yaml
│       └── main.tf                # Your Terraform code
└── README.md
```

## 🤝 Contributing

Contributions welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## 📄 License

[License details]

---

**Managed by Envie** - For more information, see the [documentation](docs/README.md).
