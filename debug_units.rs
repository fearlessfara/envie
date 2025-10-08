use envie::common::unit_discovery::UnitDiscovery;
use std::path::PathBuf;

fn main() {
    let project_root = PathBuf::from("/Users/christiangennarofaraone/projects/envie/test-full-app");
    let discovery = UnitDiscovery::new(project_root).unwrap();
    
    println!("All units:");
    for unit in discovery.registry.get_all_units() {
        println!("  - {} (level: {})", unit.config.name, unit.level);
    }
    
    println!("\nRoot units:");
    for unit in discovery.get_root_units() {
        println!("  - {} (level: {})", unit.config.name, unit.level);
    }
    
    println!("\nUnits in dependency order:");
    match discovery.get_units_in_dependency_order() {
        Ok(units) => {
            println!("  Found {} units", units.len());
            for unit in units {
                println!("  - {} (level: {})", unit.config.name, unit.level);
            }
        }
        Err(e) => println!("  Error: {}", e),
    }
}
