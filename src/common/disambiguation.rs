use crate::common::unit_config::DiscoveredUnit;
use crate::common::{EnvieError, Result};
use std::io::{self, Write};

/// Interactively disambiguate between multiple units
pub fn prompt_unit_selection<'a>(
    matches: &[&'a DiscoveredUnit],
    requested_name: &str,
) -> Result<&'a DiscoveredUnit> {
    if matches.is_empty() {
        return Err(EnvieError::ValidationError(format!(
            "No unit found matching: {}",
            requested_name
        )));
    }

    if matches.len() == 1 {
        return Ok(matches[0]);
    }

    // Multiple matches - prompt user
    println!("\n⚠️  Multiple units named '{}' found:\n", requested_name);

    for (i, unit) in matches.iter().enumerate() {
        println!(
            "  {}. {} ({:?})",
            i + 1,
            unit.qualified_name,
            unit.config.unit_type
        );
        if !unit.config.description.is_empty() {
            println!("     {}", unit.config.description);
        }
    }

    print!("\nPlease select which unit to use [1-{}]: ", matches.len());
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let selection: usize = input
        .trim()
        .parse()
        .map_err(|_| EnvieError::ValidationError("Invalid selection".to_string()))?;

    if selection < 1 || selection > matches.len() {
        return Err(EnvieError::ValidationError(format!(
            "Selection must be between 1 and {}",
            matches.len()
        )));
    }

    Ok(matches[selection - 1])
}

/// Resolve a unit with automatic disambiguation
/// If no_prompt is true, will error on ambiguity instead of prompting
/// Returns either a single unit or multiple units (for path-based operations)
pub fn resolve_unit_with_prompt<'a>(
    matches: Vec<&'a DiscoveredUnit>,
    requested_name: &str,
    no_prompt: bool,
) -> Result<&'a DiscoveredUnit> {
    if matches.is_empty() {
        return Err(EnvieError::ValidationError(format!(
            "No unit found matching: '{}'",
            requested_name
        )));
    }

    if matches.len() == 1 {
        return Ok(matches[0]);
    }

    // Multiple matches
    if no_prompt {
        // In no-prompt mode, error with helpful message
        let mut error_msg = format!(
            "Ambiguous unit name '{}'. Multiple units found:\n",
            requested_name
        );
        for unit in &matches {
            error_msg.push_str(&format!("  - {}\n", unit.qualified_name));
        }
        error_msg.push_str("\nPlease specify the full qualified name using --unit <qualified-name>");

        return Err(EnvieError::ValidationError(error_msg));
    }

    // Interactive mode - prompt user
    prompt_unit_selection(&matches, requested_name)
}

/// Resolve units with support for multiple units (path-based groups)
/// Returns a vector of units to operate on
pub fn resolve_units_with_prompt<'a>(
    matches: Vec<&'a DiscoveredUnit>,
    requested_name: &str,
    no_prompt: bool,
) -> Result<Vec<&'a DiscoveredUnit>> {
    if matches.is_empty() {
        return Err(EnvieError::ValidationError(format!(
            "No unit found matching: '{}'",
            requested_name
        )));
    }

    if matches.len() == 1 {
        return Ok(matches);
    }

    // Multiple matches - check if this looks like a path prefix
    // If all units share a common prefix that matches the request, treat as group operation
    let all_share_prefix = matches
        .iter()
        .all(|unit| unit.qualified_name.starts_with(requested_name));

    if all_share_prefix {
        // This is a path-based group operation
        if !no_prompt {
            println!(
                "\n📦 Found {} units under '{}':\n",
                matches.len(),
                requested_name
            );
            for unit in &matches {
                println!("  • {} ({:?})", unit.qualified_name, unit.config.unit_type);
                if !unit.config.description.is_empty() {
                    println!("    {}", unit.config.description);
                }
            }
            println!();
        }
        return Ok(matches);
    }

    // Ambiguous - user needs to select one or specify better path
    if no_prompt {
        let mut error_msg = format!(
            "Ambiguous unit name '{}'. Multiple units found:\n",
            requested_name
        );
        for unit in &matches {
            error_msg.push_str(&format!("  - {}\n", unit.qualified_name));
        }
        error_msg.push_str("\nPlease specify the full qualified name or path prefix using --unit <name>");

        return Err(EnvieError::ValidationError(error_msg));
    }

    // Interactive mode - let user choose one
    let selected = prompt_unit_selection(&matches, requested_name)?;
    Ok(vec![selected])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::unit_config::{DiscoveredUnit, UnitConfig, UnitType};
    use std::path::PathBuf;

    #[test]
    fn test_resolve_single_match() {
        let config = UnitConfig {
            name: "api".to_string(),
            description: "API Service".to_string(),
            unit_type: UnitType::Service,
            path: "services/api".to_string(),
            depends: vec![],
            state_management: crate::common::unit_config::StateManagement::Dedicated,
            metadata: std::collections::HashMap::new(),
        };

        let unit = DiscoveredUnit::new(config, PathBuf::from("services/api"), 1);
        let matches = vec![&unit];

        let result = resolve_unit_with_prompt(matches, "api", true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_resolve_no_matches() {
        let matches: Vec<&DiscoveredUnit> = vec![];
        let result = resolve_unit_with_prompt(matches, "nonexistent", true);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_ambiguous_no_prompt() {
        let config1 = UnitConfig {
            name: "api".to_string(),
            description: "Backend API".to_string(),
            unit_type: UnitType::Service,
            path: "services/backend/api".to_string(),
            depends: vec![],
            state_management: crate::common::unit_config::StateManagement::Dedicated,
            metadata: std::collections::HashMap::new(),
        };

        let config2 = UnitConfig {
            name: "api".to_string(),
            description: "Frontend API".to_string(),
            unit_type: UnitType::Service,
            path: "services/frontend/api".to_string(),
            depends: vec![],
            state_management: crate::common::unit_config::StateManagement::Dedicated,
            metadata: std::collections::HashMap::new(),
        };

        let unit1 = DiscoveredUnit::new(config1, PathBuf::from("services/backend/api"), 2);
        let unit2 = DiscoveredUnit::new(config2, PathBuf::from("services/frontend/api"), 2);

        let matches = vec![&unit1, &unit2];
        let result = resolve_unit_with_prompt(matches, "api", true);

        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("Ambiguous"));
        assert!(err_msg.contains("services/backend/api"));
        assert!(err_msg.contains("services/frontend/api"));
    }
}
