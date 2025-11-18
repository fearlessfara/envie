use crate::common::unit_config::DiscoveredUnit;
use crate::common::{EnvieError, Result};
use std::io::{self, Write};

/// Calculate Levenshtein distance for fuzzy matching
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.len();
    let len2 = s2.len();
    let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

    for i in 0..=len1 {
        matrix[i][0] = i;
    }
    for j in 0..=len2 {
        matrix[0][j] = j;
    }

    for i in 1..=len1 {
        for j in 1..=len2 {
            let cost = if s1.chars().nth(i - 1) == s2.chars().nth(j - 1) {
                0
            } else {
                1
            };
            matrix[i][j] = std::cmp::min(
                std::cmp::min(matrix[i - 1][j] + 1, matrix[i][j - 1] + 1),
                matrix[i - 1][j - 1] + cost,
            );
        }
    }

    matrix[len1][len2]
}

/// Find similar units using fuzzy matching
fn find_similar_units<'a>(
    all_units: &[&'a DiscoveredUnit],
    requested_name: &str,
) -> Vec<(&'a DiscoveredUnit, usize)> {
    let mut similarities: Vec<_> = all_units
        .iter()
        .map(|unit| {
            // Check similarity against both name and qualified_name
            let name_distance = levenshtein_distance(requested_name, &unit.config.name);
            let qualified_distance =
                levenshtein_distance(requested_name, &unit.qualified_name);
            let best_distance = std::cmp::min(name_distance, qualified_distance);
            (*unit, best_distance)
        })
        .filter(|(_, distance)| *distance <= 3) // Only show if distance <= 3
        .collect();

    similarities.sort_by_key(|(_, distance)| *distance);
    similarities.truncate(5); // Show top 5 suggestions
    similarities
}

/// Create a helpful "unit not found" error message with suggestions
fn create_unit_not_found_error(
    requested_name: &str,
    all_units: &[&DiscoveredUnit],
) -> EnvieError {
    let mut error_msg = format!("❌ Unit '{}' not found\n", requested_name);

    // Find similar units
    let similar = find_similar_units(all_units, requested_name);

    if !similar.is_empty() {
        error_msg.push_str("\n💡 Did you mean one of these?\n");
        for (unit, distance) in similar {
            let similarity_pct = 100 - (distance * 100 / requested_name.len().max(1));
            error_msg.push_str(&format!(
                "   • {} (similarity: {}%)\n",
                unit.qualified_name, similarity_pct
            ));
            if !unit.config.description.is_empty() {
                error_msg.push_str(&format!("     {}\n", unit.config.description));
            }
        }
    }

    error_msg.push_str("\n💡 To see all available units:\n");
    error_msg.push_str("   envie list\n");

    error_msg.push_str("\n💡 To search for units:\n");
    error_msg.push_str(&format!("   envie show --search {}\n", requested_name));

    EnvieError::ValidationError(error_msg)
}

/// Interactively disambiguate between multiple units
pub fn prompt_unit_selection<'a>(
    matches: &[&'a DiscoveredUnit],
    requested_name: &str,
    all_units: &[&'a DiscoveredUnit],
) -> Result<&'a DiscoveredUnit> {
    if matches.is_empty() {
        return Err(create_unit_not_found_error(requested_name, all_units));
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
    all_units: &[&'a DiscoveredUnit],
    requested_name: &str,
    no_prompt: bool,
) -> Result<&'a DiscoveredUnit> {
    if matches.is_empty() {
        return Err(create_unit_not_found_error(requested_name, all_units));
    }

    if matches.len() == 1 {
        return Ok(matches[0]);
    }

    // Multiple matches
    if no_prompt {
        // In no-prompt mode, error with helpful message
        let mut error_msg = format!(
            "⚠️  Ambiguous unit name '{}'\n\n",
            requested_name
        );
        error_msg.push_str("Multiple units found:\n");
        for unit in &matches {
            error_msg.push_str(&format!("   • {}\n", unit.qualified_name));
            if !unit.config.description.is_empty() {
                error_msg.push_str(&format!("     {}\n", unit.config.description));
            }
        }
        error_msg.push_str("\n💡 How to fix:\n");
        error_msg.push_str("   Use the full qualified name:\n");
        error_msg.push_str(&format!("   envie deploy --unit {} --env <env-id>\n", matches[0].qualified_name));
        error_msg.push_str("\n   Or run without --no-prompt to choose interactively\n");

        return Err(EnvieError::ValidationError(error_msg));
    }

    // Interactive mode - prompt user
    prompt_unit_selection(&matches, requested_name, all_units)
}

/// Resolve units with support for multiple units (path-based groups)
/// Returns a vector of units to operate on
pub fn resolve_units_with_prompt<'a>(
    matches: Vec<&'a DiscoveredUnit>,
    all_units: &[&'a DiscoveredUnit],
    requested_name: &str,
    no_prompt: bool,
) -> Result<Vec<&'a DiscoveredUnit>> {
    if matches.is_empty() {
        return Err(create_unit_not_found_error(requested_name, all_units));
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
            "⚠️  Ambiguous unit name '{}'\n\n",
            requested_name
        );
        error_msg.push_str("Multiple units found:\n");
        for unit in &matches {
            error_msg.push_str(&format!("   • {}\n", unit.qualified_name));
            if !unit.config.description.is_empty() {
                error_msg.push_str(&format!("     {}\n", unit.config.description));
            }
        }
        error_msg.push_str("\n💡 How to fix:\n");
        error_msg.push_str("   Use a more specific path or qualified name:\n");
        error_msg.push_str(&format!("   envie deploy --unit {} --env <env-id>\n", matches[0].qualified_name));
        error_msg.push_str("\n   Or run without --no-prompt to choose interactively\n");

        return Err(EnvieError::ValidationError(error_msg));
    }

    // Interactive mode - let user choose one
    let selected = prompt_unit_selection(&matches, requested_name, all_units)?;
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
        let all_units = vec![&unit];

        let result = resolve_unit_with_prompt(matches, &all_units, "api", true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_resolve_no_matches() {
        let config = UnitConfig {
            name: "database".to_string(),
            description: "Database".to_string(),
            unit_type: UnitType::Service,
            path: "services/database".to_string(),
            depends: vec![],
            state_management: crate::common::unit_config::StateManagement::Dedicated,
            metadata: std::collections::HashMap::new(),
        };
        let unit = DiscoveredUnit::new(config, PathBuf::from("services/database"), 1);
        let matches: Vec<&DiscoveredUnit> = vec![];
        let all_units = vec![&unit];

        let result = resolve_unit_with_prompt(matches, &all_units, "nonexistent", true);
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
        let all_units = vec![&unit1, &unit2];
        let result = resolve_unit_with_prompt(matches, &all_units, "api", true);

        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("Ambiguous"));
        assert!(err_msg.contains("services/backend/api"));
        assert!(err_msg.contains("services/frontend/api"));
    }
}
