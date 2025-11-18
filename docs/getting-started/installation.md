# Installation

This guide will help you install Envie and its prerequisites.

## Prerequisites

### Required
- **Terraform** >=  1.0
  - [Download Terraform](https://www.terraform.io/downloads)
  - Verify: `terraform --version`

- **Rust** (for building from source)
  - [Install Rust](https://rustup.rs/)
  - Verify: `cargo --version`

### Recommended
- **Git** - For version control
  - Verify: `git --version`

- **AWS CLI** - For AWS operations
  - [Install AWS CLI](https://aws.amazon.com/cli/)
  - Configure: `aws configure`
  - Verify: `aws sts get-caller-identity`

## Installation Methods

### From Source (Current)

```bash
# Clone the repository
git clone https://github.com/your-org/envie.git
cd envie

# Build the project
cargo build --release

# The binary will be at target/release/envie
# Add to your PATH:
export PATH="$PWD/target/release:$PATH"

# Verify installation
envie --version
```

### Using Cargo (Future)

```bash
# Install from crates.io
cargo install envie

# Verify installation
envie --version
```

### Using Homebrew (Future)

```bash
# Install via Homebrew
brew install envie

# Verify installation
envie --version
```

## Post-Installation

### 1. Verify Installation

Run the health check to ensure everything is set up correctly:

```bash
envie doctor
```

You should see output like:

```
🏥 Running Envie Health Checks

Prerequisites:
  ✅ Terraform installed
     Terraform v1.5.0
  ✅ Git installed
     git version 2.40.0
  ⚠️  AWS credentials configured
     AWS credentials found

Summary:
  Total checks: 3
  ✅ Passed: 3

Overall: ✅ Healthy
```

### 2. Configure AWS Credentials

If you haven't already, configure your AWS credentials:

```bash
# Using AWS CLI
aws configure

# Or set environment variables
export AWS_ACCESS_KEY_ID=your_access_key
export AWS_SECRET_ACCESS_KEY=your_secret_key
export AWS_DEFAULT_REGION=us-east-1
```

### 3. Verify Terraform

Ensure Terraform is installed and in your PATH:

```bash
terraform --version
```

Should output something like:
```
Terraform v1.5.0
on linux_amd64
```

## Troubleshooting

### Terraform Not Found

**Error**: `Terraform not found in PATH`

**Solution**:
1. Download Terraform from https://www.terraform.io/downloads
2. Add to your PATH:
   ```bash
   # On Linux/macOS
   export PATH="$PATH:/path/to/terraform"

   # On Windows
   set PATH=%PATH%;C:\path\to\terraform
   ```

### AWS Credentials Not Configured

**Error**: `No AWS credentials found`

**Solution**:
```bash
# Option 1: Use AWS CLI
aws configure

# Option 2: Set environment variables
export AWS_ACCESS_KEY_ID=your_key
export AWS_SECRET_ACCESS_KEY=your_secret

# Option 3: Use AWS profiles
export AWS_PROFILE=your_profile
```

### Build Errors

**Error**: Rust compiler errors

**Solution**:
1. Update Rust: `rustup update`
2. Clean and rebuild: `cargo clean && cargo build --release`

## Next Steps

Now that you have Envie installed, let's get started!

**Next**: [Quick Start Guide](quickstart.md) →

Or jump to:
- [Core Concepts](concepts.md) - Understand how Envie works
- [CLI Commands](../reference/commands.md) - Command reference
