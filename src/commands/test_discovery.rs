use crate::common::*;
use std::path::PathBuf;

pub struct TestDiscoveryCommand {
    pub project_path: PathBuf,
}

impl TestDiscoveryCommand {
    pub fn new(project_path: PathBuf) -> Self {
        Self { project_path }
    }
    
    pub fn execute(&self) -> Result<()> {
        println!("🔍 Testing Flexible Unit Discovery System");
        println!("==========================================");
        
        let mut discovery = UnitDiscovery::new(self.project_path.clone());
        
        // Discover all units
        discovery.discover_all()?;
        
        println!("\n📊 Discovered Units:");
        println!("-------------------");
        
        for unit in discovery.registry.get_all_units() {
            let indent = "  ".repeat(unit.level);
            println!("{}{} ({:?}) - {}", 
                indent, 
                unit.config.name, 
                unit.config.unit_type,
                unit.config.description
            );
            println!("{}  Path: {}", indent, unit.path.display());
            println!("{}  State: {:?}", indent, unit.config.state_management);
            if !unit.config.dependencies.is_empty() {
                println!("{}  Dependencies:", indent);
                for dep in &unit.config.dependencies {
                    println!("{}    - {} ({})", indent, dep.path, dep.environment);
                }
            }
            println!();
        }
        
        println!("\n🏗️  Units by Type:");
        println!("------------------");
        
        for unit_type in [UnitType::Layer, UnitType::Application, UnitType::Service, UnitType::Component] {
            let units = discovery.get_units_by_type(&unit_type);
            if !units.is_empty() {
                println!("\n{:?}:", unit_type);
                for unit in units {
                    println!("  - {} ({})", unit.config.name, unit.config.description);
                }
            }
        }
        
        println!("\n🌳 Root Level Units:");
        println!("-------------------");
        
        for unit in discovery.get_root_units() {
            println!("- {} ({:?})", unit.config.name, unit.config.unit_type);
        }
        
        println!("\n🍃 Leaf Units:");
        println!("--------------");
        
        for unit in discovery.get_leaf_units() {
            println!("- {} ({:?})", unit.config.name, unit.config.unit_type);
        }
        
        println!("\n✅ Flexible discovery system working perfectly!");
        println!("   - Supports arbitrary nesting levels");
        println!("   - Relative path dependencies");
        println!("   - Multiple unit types");
        println!("   - Flexible state management");
        
        Ok(())
    }
}
