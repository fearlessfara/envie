use std::fs;

fn main() {
    // Read the test YAML
    let yaml_content = fs::read_to_string("test_dependency_resolution.yaml")
        .expect("Failed to read test YAML");
    
    // Parse it (this would use the actual config module)
    println!("YAML content:");
    println!("{}", yaml_content);
    println!("\n=== Expected Deployment Order for 'frontend' ===");
    println!("1. networking/vpc");
    println!("2. networking/security-groups");
    println!("3. database/dynamodb (or cache/redis)");
    println!("4. cache/redis (or database/dynamodb)");
    println!("5. api/lambda");
    println!("6. api/gateway");
    println!("7. frontend/cdn");
    println!("\nThis tests:");
    println!("- Module-level deps: gateway depends on lambda");
    println!("- Service-level deps: api depends on [database, cache]");
    println!("- Transitive deps: database and cache both depend on networking");
    println!("- Diamond dependency: frontend -> api -> {database, cache} -> networking");
}

