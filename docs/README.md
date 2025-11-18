# Envie Documentation

Welcome to Envie! This documentation will help you get started with managing multiple ephemeral environments in Terraform.

## 📚 Documentation Structure

### Getting Started
- [Installation](getting-started/installation.md) - Install and set up Envie
- [Quick Start](getting-started/quickstart.md) - Your first deployment in 15 minutes
- [Core Concepts](getting-started/concepts.md) - Understanding environments, units, and dependencies

### Guides
- [Environment Overrides](guides/environment-overrides.md) - Using the `-E` flag for flexible deployments
- [Dependency Management](guides/dependency-management.md) - Managing cross-unit dependencies
- [Workflows](guides/workflows.md) - Common development workflows
- [CI/CD Integration](guides/cicd-integration.md) - Using Envie in your pipeline

### Reference
- [CLI Commands](reference/commands.md) - Complete command reference
- [Configuration](reference/configuration.md) - workspace.envie.yaml and envie.yaml reference
- [Troubleshooting](reference/troubleshooting.md) - Common issues and solutions

### Examples
- [Full Backend Example](../examples/full_backend/README.md) - Complete serverless backend with API Gateway, Lambda, and DynamoDB

## 🚀 Quick Links

**New to Envie?** Start with the [Quick Start Guide](getting-started/quickstart.md)

**Need help with a specific task?**
- Deploying to ephemeral environments → [Workflows](guides/workflows.md)
- Mixing stable and ephemeral resources → [Environment Overrides](guides/environment-overrides.md)
- Setting up CI/CD → [CI/CD Integration](guides/cicd-integration.md)

**Looking for command syntax?** → [CLI Commands Reference](reference/commands.md)

**Having issues?** → [Troubleshooting](reference/troubleshooting.md) or run `envie doctor`

## 📖 Table of Contents

1. [Getting Started](#getting-started)
   - Installation
   - Quick Start (15 min)
   - Core Concepts

2. [Guides](#guides)
   - Environment Overrides
   - Dependency Management
   - Common Workflows
   - CI/CD Integration

3. [Reference](#reference)
   - CLI Commands
   - Configuration Files
   - Troubleshooting

4. [Examples](#examples)
   - Full Backend Example
   - More examples coming soon!

## 🆘 Getting Help

- **Run health checks**: `envie doctor`
- **View command help**: `envie <command> --help`
- **Report issues**: [GitHub Issues](https://github.com/your-org/envie/issues)

## 💡 Pro Tips

1. Always run `envie doctor` after setup to verify your configuration
2. Use `envie plan` instead of `deploy --dry-run` for quick previews
3. Run `envie list` to see what's currently deployed
4. Use `-E` flags to override environments for testing against production data

---

**Next**: [Installation Guide](getting-started/installation.md) →
