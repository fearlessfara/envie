# Phase 1: Terraform Crate Integration - Migration Guide

## Overview

Phase 1 introduces `EnvieTerraformManager` - an enhanced Terraform manager that uses the `terraform` crate for better process management while maintaining the exact same interface as the original `TerraformManager`.

## What's New

### Enhanced Features
- **Better Process Management**: Uses the `terraform` crate for structured process handling
- **Async Support**: New async methods for better integration with async workflows
- **Improved Error Handling**: Better error messages and structured error handling
- **Event Streaming**: Built-in support for Terraform event streaming (future enhancement)

### Backward Compatibility
- **Same Interface**: All existing methods work exactly the same
- **No Breaking Changes**: Drop-in replacement for `TerraformManager`
- **Same Configuration**: Uses the same environment variables and settings

## Migration Steps

### Step 1: Update Imports

**Before:**
```rust
use crate::common::terraform::TerraformManager;
```

**After:**
```rust
use crate::common::envie_terraform_manager::EnvieTerraformManager as TerraformManager;
// Or simply:
use crate::common::EnvieTerraformManager as TerraformManager;
```

### Step 2: No Code Changes Required

The `EnvieTerraformManager` implements the exact same interface as `TerraformManager`:

```rust
// This code works exactly the same with EnvieTerraformManager
let manager = EnvieTerraformManager::new(working_directory);
manager.init()?;
manager.workspace_new("my-workspace")?;
manager.apply(&[("key", "value")])?;
```

### Step 3: Optional - Use New Async Methods

For better performance in async contexts, you can use the new async methods:

```rust
// New async methods available
manager.init_async().await?;
manager.apply_async(&[("key", "value")], "plan.tfplan").await?;
manager.destroy_async().await?;
```

## Benefits

### 1. Better Error Handling
**Before:**
```
Error: terraform apply failed: exit code 1
```

**After:**
```
Error: terraform apply failed: 
Error: Resource aws_instance.web already exists
Error: Resource aws_security_group.web already exists
```

### 2. Structured Process Management
- Better timeout handling
- Improved process monitoring
- Cleaner resource cleanup

### 3. Future-Ready
- Ready for event streaming
- Better integration with monitoring tools
- Enhanced debugging capabilities

## Testing

The new manager includes comprehensive tests:

```bash
cargo test envie_terraform_manager
```

## Rollback Plan

If issues arise, you can easily rollback by:

1. Reverting the import changes
2. The original `TerraformManager` remains available
3. No data or configuration changes required

## Next Steps

After successful Phase 1 migration:
- Phase 2: Enhanced service discovery with `tfconfig`
- Phase 3: Registry validation with `terraform-registry`

## Example Usage

```rust
use crate::common::EnvieTerraformManager;

#[tokio::main]
async fn main() -> Result<()> {
    let manager = EnvieTerraformManager::new("/path/to/terraform")
        .with_verbose(true);

    // Traditional sync methods (same as before)
    manager.init()?;
    manager.workspace_new("my-env")?;
    manager.apply(&[("environment", "dev")])?;

    // New async methods (better for async workflows)
    manager.init_async().await?;
    manager.apply_async(&[("environment", "dev")], "plan.tfplan").await?;

    Ok(())
}
```

## Support

For questions or issues with the migration:
- Check the test suite for usage examples
- Review the error messages for better debugging
- The original `TerraformManager` remains available as fallback
