# Troubleshooting

Common issues and their solutions.

> 💡 **First step**: Run `envie doctor` to diagnose common problems

## Quick Diagnostics

```bash
# Run health checks
envie doctor

# Check what's deployed
envie list

# Preview before deploying
envie plan --unit api --env test

# Verbose output for debugging
envie deploy --unit api --env test --verbose
```

---

## Installation Issues

### Terraform Not Found

**Error**: `Terraform not found in PATH`

**Solution**:
1. Install Terraform: https://www.terraform.io/downloads
2. Add to PATH:
   ```bash
   export PATH="$PATH:/path/to/terraform"
   ```
3. Verify: `terraform --version`

### AWS Credentials Not Configured

**Error**: `No AWS credentials found`

**Solutions**:
```bash
# Option 1: AWS CLI
aws configure

# Option 2: Environment variables
export AWS_ACCESS_KEY_ID=your_key
export AWS_SECRET_ACCESS_KEY=your_secret
export AWS_DEFAULT_REGION=us-east-1

# Option 3: AWS Profile
export AWS_PROFILE=your_profile
```

---

## Configuration Issues

### workspace.envie.yaml Not Found

**Error**: `workspace.envie.yaml exists ❌`

**Solution**:
```bash
# Initialize project
envie init --name myproject

# Or create manually (see configuration docs)
```

### Invalid YAML Syntax

**Error**: `YAML parsing error`

**Solution**:
1. Check indentation (use spaces, not tabs)
2. Validate YAML: https://www.yamllint.com/
3. Check quotes around special characters

**Example**:
```yaml
# ✓ Correct
name: my-api
description: "API: handles requests"

# ✗ Wrong (unquoted special chars)
name: my-api
description: API: handles requests
```

### Invalid Environment Reference

**Error**: `Invalid stable environment 'stable.sanbox'`

**Solution**:
1. Check spelling in `envie.yaml`
2. Verify environment exists in `workspace.envie.yaml`
3. Available environments shown in error message

---

## Deployment Issues

### Unit Not Found

**Error**: `❌ Unit 'ap' not found`

**Possible causes**:
1. Typo in unit name
2. Wrong directory
3. Unit not discovered

**Solutions**:
```bash
# List all units
envie list

# Check from project root
cd /path/to/project
envie deploy --unit api --env test

# Use full path
envie deploy --unit services/api --env test
```

### Ambiguous Unit Name

**Error**: `⚠️  Ambiguous unit name 'api'`

**Solution**:
Use the full qualified name shown in the error:
```bash
envie deploy --unit services/backend/api --env test
```

### Circular Dependency

**Error**: `Circular dependency detected`

**Solution**:
1. Run `envie doctor` to see dependency graph
2. Remove circular reference in `envie.yaml`
3. Restructure dependencies

**Example**:
```yaml
# ✗ Circular (api → database → api)
# api/envie.yaml
depends:
  - path: ../database

# database/envie.yaml
depends:
  - path: ../api  # ← Remove this

# ✓ Fixed (api → database, no reverse)
# api/envie.yaml
depends:
  - path: ../database

# database/envie.yaml
depends: []  # No dependencies
```

### State Locking Error

**Error**: `Error acquiring the state lock`

**Possible causes**:
1. Another deployment is running
2. Previous deployment crashed
3. Lock not released

**Solutions**:
```bash
# Wait for other deployment to finish

# If stuck, force unlock (use carefully!)
cd path/to/unit
terraform force-unlock <LOCK_ID>

# Or clean and reinitialize
envie clean --unit api
```

---

## Terraform Errors

### Backend Initialization Failed

**Error**: `Failed to configure backend`

**Solutions**:
```bash
# Check AWS credentials
aws sts get-caller-identity

# Check S3 bucket exists
aws s3 ls s3://your-state-bucket

# Verify bucket permissions
aws s3api get-bucket-policy --bucket your-state-bucket

# Clean and retry
envie clean --unit api
envie deploy --unit api --env test
```

### Module Not Found

**Error**: `Module not found`

**Solutions**:
```bash
# Reinitialize Terraform
envie clean --unit api --upgrade

# Check module source in .tf files
# Verify module exists at specified path
```

### Resource Already Exists

**Error**: `AlreadyExists: Resource 'xyz' already exists`

**Possible causes**:
1. Resource created outside Terraform
2. State file out of sync
3. Workspace confusion

**Solutions**:
```bash
# Import existing resource
cd path/to/unit
terraform import <resource_type>.<name> <resource_id>

# Or remove conflicting resource manually (AWS Console)

# Then redeploy
envie deploy --unit api --env test
```

---

## Environment Issues

### Wrong Environment Deployed

**Problem**: Deployed to wrong environment accidentally

**Prevention**:
```bash
# Always use dry-run first
envie plan --unit api --env test

# Use verbose to verify
envie deploy --unit api --env test --verbose
```

**Recovery**:
```bash
# Destroy wrong environment
envie destroy --env wrong-env

# Deploy to correct environment
envie deploy --unit api --env correct-env
```

### Mixed Environments Not Working

**Problem**: Can't use stable and ephemeral together

**Solution**:
```bash
# Use -E flag to override
envie deploy --unit api --env dev-123 \\
  -E database:stable.sandbox \\
  -E networking:ephemeral

# Verify with verbose
envie deploy --unit api --env dev-123 --verbose
```

---

## Performance Issues

### Slow Deployments

**Causes**:
1. Large Terraform state
2. Many resources
3. Slow provider operations

**Solutions**:
```bash
# Deploy specific unit only
envie deploy --unit api --env test

# Use --no-prompt to skip confirmations
envie deploy --unit api --env test --no-prompt

# Parallelize if possible (separate units)
envie deploy --unit networking --env test &
envie deploy --unit database --env test &
wait
envie deploy --unit api --env test
```

### High AWS Costs

**Problem**: Forgotten ephemeral environments accumulating costs

**Solutions**:
```bash
# List all environments
envie list

# Delete old environments
envie delete --env old-feature-123
envie delete --env dev-abc

# Automated cleanup (CI/CD)
# Delete environments older than 7 days
```

---

## Generated Files Issues

### envie-*.tf Files Not Generated

**Problem**: Expected files not created

**Checks**:
```bash
# Verify unit discovery
envie list

# Check envie.yaml exists
ls services/api/envie.yaml

# Try verbose deployment
envie deploy --unit api --env test --verbose
```

### Git Tracking Generated Files

**Problem**: Generated files committed to git

**Solution**:
```bash
# Ensure .gitignore has patterns
echo "envie-backend.tf" >> .gitignore
echo "envie-remote-state.tf" >> .gitignore

# Remove from git
git rm --cached services/*/envie-*.tf
git commit -m "Remove generated files"
```

---

## Debug Techniques

### Enable Verbose Mode

```bash
# See what Envie is doing
envie deploy --unit api --env test --verbose
```

### Check Terraform Directly

```bash
# Navigate to unit
cd services/api

# Check Terraform plan
terraform plan

# Check state
terraform show

# List workspaces
terraform workspace list
```

### Inspect Generated Files

```bash
# Check backend config
cat services/api/envie-backend.tf

# Check remote state
cat services/api/envie-remote-state.tf
```

### AWS Console Verification

1. Check S3 bucket for state files
2. Check DynamoDB for locks
3. Verify deployed resources exist

---

## Getting More Help

### Documentation

- [Quick Start](../getting-started/quickstart.md)
- [CLI Commands](commands.md)
- [Workflows](../guides/workflows.md)

### Support Channels

- Run `envie doctor` for diagnostics
- Check GitHub Issues
- Review error messages carefully (they include suggestions!)

### Reporting Bugs

When reporting issues, include:

1. **Envie version**: `envie --version`
2. **Command run**: Full command with flags
3. **Error message**: Complete error output
4. **Doctor output**: `envie doctor` results
5. **Environment**: OS, Terraform version, AWS region

```bash
# Collect debug info
envie --version > debug.txt
envie doctor >> debug.txt 2>&1
terraform --version >> debug.txt
aws --version >> debug.txt
```

---

**Previous**: [Configuration](configuration.md) | **Next**: [Commands](commands.md)
