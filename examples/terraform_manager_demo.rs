use envie::common::EnvieTerraformManager;
use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 EnvieTerraformManager Demo");
    println!("=============================");

    // Create a temporary directory for this demo
    let temp_dir = TempDir::new()?;
    let working_dir = temp_dir.path();

    println!("📁 Working directory: {:?}", working_dir);

    // Create the new enhanced Terraform manager
    let manager = EnvieTerraformManager::new(working_dir)
        .with_verbose(true);

    println!("✅ Created EnvieTerraformManager");

    // Demonstrate workspace management (same interface as before)
    println!("\n🔧 Workspace Management:");
    
    // List workspaces
    match manager.workspace_list() {
        Ok(workspaces) => {
            println!("📋 Current workspaces: {:?}", workspaces);
        }
        Err(e) => {
            println!("⚠️  Could not list workspaces (expected if not initialized): {}", e);
        }
    }

    // Show current workspace
    match manager.workspace_show() {
        Ok(workspace) => {
            println!("📍 Current workspace: {}", workspace);
        }
        Err(e) => {
            println!("⚠️  Could not show workspace (expected if not initialized): {}", e);
        }
    }

    // Demonstrate async methods (new feature)
    println!("\n⚡ Async Methods Demo:");
    
    // Note: These would normally work with actual Terraform files
    // For demo purposes, we'll show the interface
    println!("🔄 init_async() - Available for better async integration");
    println!("🔄 apply_async() - Available for better async integration");
    println!("🔄 destroy_async() - Available for better async integration");

    // Demonstrate error handling improvements
    println!("\n🛡️  Enhanced Error Handling:");
    println!("   - Better error messages");
    println!("   - Structured error types");
    println!("   - Improved debugging information");

    // Demonstrate backward compatibility
    println!("\n🔄 Backward Compatibility:");
    println!("   - Same interface as TerraformManager");
    println!("   - Drop-in replacement");
    println!("   - No breaking changes");

    println!("\n✨ Demo completed successfully!");
    println!("\nNext steps:");
    println!("1. Replace TerraformManager imports with EnvieTerraformManager");
    println!("2. Optionally use new async methods for better performance");
    println!("3. Enjoy improved error handling and process management");

    Ok(())
}
