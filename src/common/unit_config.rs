use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// A unit represents any deployable component in the system
/// It can be a service, module, component, layer, application, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitConfig {
    /// The name of the unit
    pub name: String,
    
    /// Optional description
    #[serde(default)]
    pub description: String,
    
    /// The type of unit (service, module, component, layer, application, etc.)
    #[serde(default)]
    pub unit_type: UnitType,
    
    /// Path to this unit's directory (relative to project root)
    #[serde(default)]
    pub path: String,
    
    /// Dependencies on other units
    #[serde(default)]
    pub depends: Vec<DependencyReference>,
    
    /// State management strategy
    #[serde(default)]
    pub state_management: StateManagement,
    
    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum UnitType {
    /// A top-level service (e.g., API, Database, Frontend)
    Service,
    /// A module within a service (e.g., Lambda, DynamoDB)
    Module,
    /// A component (e.g., VPC, Subnet, Security Group)
    Component,
    /// An infrastructure layer (e.g., Networking, Compute, Data)
    Layer,
    /// An application (e.g., Web App, Mobile App)
    Application,
    /// A custom type
    Custom(String),
}

impl Default for UnitType {
    fn default() -> Self {
        UnitType::Module
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyReference {
    /// Relative path to the dependency (e.g., "../networking/vpc", "./auth")
    pub path: String,
    
    /// Environment to use for this dependency
    pub environment: String,
    
    /// Optional alias for the dependency
    #[serde(default)]
    pub alias: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(from = "StateManagementString")]
pub enum StateManagement {
    /// Unit has its own dedicated state file
    Dedicated,
    /// Unit is managed as part of a parent unit's state
    Parent,
    /// Unit is managed as part of a shared state with other units
    Shared(String), // The shared state identifier
    /// Unit is managed as part of a group state
    Group(String), // The group identifier
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum StateManagementString {
    String(String),
    Object { shared: String },
    ObjectGroup { group: String },
}

impl From<StateManagementString> for StateManagement {
    fn from(s: StateManagementString) -> Self {
        match s {
            StateManagementString::String(s) => {
                if s == "dedicated" {
                    StateManagement::Dedicated
                } else if s == "parent" {
                    StateManagement::Parent
                } else if s.starts_with("shared:") {
                    StateManagement::Shared(s.strip_prefix("shared:").unwrap().to_string())
                } else if s.starts_with("group:") {
                    StateManagement::Group(s.strip_prefix("group:").unwrap().to_string())
                } else {
                    StateManagement::Dedicated // Default fallback
                }
            }
            StateManagementString::Object { shared } => {
                StateManagement::Shared(shared)
            }
            StateManagementString::ObjectGroup { group } => {
                StateManagement::Group(group)
            }
        }
    }
}

impl Default for StateManagement {
    fn default() -> Self {
        StateManagement::Dedicated
    }
}

impl StateManagement {
    pub fn is_dedicated(&self) -> bool {
        matches!(self, StateManagement::Dedicated)
    }
    
    pub fn is_parent(&self) -> bool {
        matches!(self, StateManagement::Parent)
    }
    
    pub fn is_shared(&self) -> bool {
        matches!(self, StateManagement::Shared(_))
    }
    
    pub fn is_group(&self) -> bool {
        matches!(self, StateManagement::Group(_))
    }
    
    pub fn shared_id(&self) -> Option<&String> {
        match self {
            StateManagement::Shared(id) => Some(id),
            _ => None,
        }
    }
    
    pub fn group_id(&self) -> Option<&String> {
        match self {
            StateManagement::Group(id) => Some(id),
            _ => None,
        }
    }
}

/// A discovered unit in the project
#[derive(Debug, Clone)]
pub struct DiscoveredUnit {
    pub config: UnitConfig,
    pub path: PathBuf,
    pub qualified_name: String, // Full path-based name for disambiguation
    pub level: usize, // Depth in the directory structure
    pub parent: Option<PathBuf>,
    pub children: Vec<PathBuf>,
}

impl DiscoveredUnit {
    pub fn new(config: UnitConfig, path: PathBuf, level: usize) -> Self {
        // Generate qualified name from path
        let qualified_name = path.to_string_lossy().replace('\\', "/");

        Self {
            config,
            path,
            qualified_name,
            level,
            parent: None,
            children: Vec::new(),
        }
    }
    
    pub fn is_root_level(&self) -> bool {
        self.level == 0
    }
    
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }
    
    pub fn get_relative_path_to(&self, other: &DiscoveredUnit) -> String {
        // Calculate relative path from this unit to another unit
        let self_path = &self.path;
        let other_path = &other.path;
        
        // This is a simplified implementation
        // In practice, we'd use proper path resolution
        if other_path.starts_with(self_path) {
            other_path.strip_prefix(self_path)
                .unwrap_or(other_path)
                .to_string_lossy()
                .to_string()
        } else {
            // Calculate relative path going up the tree
            let mut relative = String::new();
            let mut current_path = self_path.clone();
            
            // Go up until we find a common ancestor
            while !other_path.starts_with(&current_path) {
                relative.push_str("../");
                if let Some(parent) = current_path.parent() {
                    current_path = parent.to_path_buf();
                } else {
                    break;
                }
            }
            
            // Add the path from common ancestor to target
            let remaining = other_path.strip_prefix(&current_path)
                .unwrap_or(other_path);
            relative.push_str(&remaining.to_string_lossy());
            
            relative
        }
    }
}

/// Registry of all discovered units in the project
#[derive(Debug, Clone)]
pub struct UnitRegistry {
    pub units: HashMap<String, Vec<DiscoveredUnit>>, // Changed to Vec to support duplicate names
    pub units_by_qualified_name: HashMap<String, DiscoveredUnit>,
    pub units_by_path: HashMap<PathBuf, String>,
    pub units_by_type: HashMap<UnitType, Vec<String>>,
}

impl UnitRegistry {
    pub fn new() -> Self {
        Self {
            units: HashMap::new(),
            units_by_qualified_name: HashMap::new(),
            units_by_path: HashMap::new(),
            units_by_type: HashMap::new(),
        }
    }

    pub fn add_unit(&mut self, unit: DiscoveredUnit) {
        let name = unit.config.name.clone();
        let qualified_name = unit.qualified_name.clone();
        let path = unit.path.clone();
        let unit_type = unit.config.unit_type.clone();

        // Add to units map (supports duplicates)
        self.units
            .entry(name.clone())
            .or_insert_with(Vec::new)
            .push(unit.clone());

        // Add to qualified name map (must be unique)
        self.units_by_qualified_name.insert(qualified_name, unit);

        // Add to path map
        self.units_by_path.insert(path, name.clone());

        // Add to type map
        self.units_by_type
            .entry(unit_type)
            .or_insert_with(Vec::new)
            .push(name);
    }
    
    /// Get a single unit by simple name (returns first match if duplicates exist)
    /// Use resolve_unit() for better disambiguation
    pub fn get_unit(&self, name: &str) -> Option<&DiscoveredUnit> {
        self.units.get(name).and_then(|units| units.first())
    }

    /// Get all units matching a simple name (may return multiple if duplicates exist)
    pub fn get_units_by_name(&self, name: &str) -> Vec<&DiscoveredUnit> {
        self.units
            .get(name)
            .map(|units| units.iter().collect())
            .unwrap_or_default()
    }

    /// Get a unit by its qualified name (guaranteed unique)
    pub fn get_unit_by_qualified_name(&self, qualified_name: &str) -> Option<&DiscoveredUnit> {
        self.units_by_qualified_name.get(qualified_name)
    }

    pub fn get_unit_by_path(&self, path: &PathBuf) -> Option<&DiscoveredUnit> {
        self.units_by_path.get(path).and_then(|name| self.get_unit(name))
    }

    pub fn get_units_by_type(&self, unit_type: &UnitType) -> Vec<&DiscoveredUnit> {
        self.units_by_type
            .get(unit_type)
            .map(|names| {
                names
                    .iter()
                    .filter_map(|name| self.get_unit(name))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn get_all_units(&self) -> Vec<&DiscoveredUnit> {
        self.units_by_qualified_name.values().collect()
    }

    pub fn find_units_by_pattern(&self, pattern: &str) -> Vec<&DiscoveredUnit> {
        self.units_by_qualified_name
            .values()
            .filter(|unit| {
                unit.config.name.contains(pattern)
                    || unit.config.description.contains(pattern)
                    || unit.qualified_name.contains(pattern)
            })
            .collect()
    }

    /// Resolve a unit name or path to specific unit(s)
    /// Returns all possible matches for disambiguation
    ///
    /// Resolution order:
    /// 1. Exact match on qualified name (e.g., "infrastructure/database/dynamodb")
    /// 2. Partial path match (e.g., "database/dynamodb")
    /// 3. Path prefix match - returns all units under that path (e.g., "infrastructure" returns all units in infrastructure/*)
    /// 4. Simple name match (e.g., "api")
    pub fn resolve_unit(&self, name_or_path: &str) -> Vec<&DiscoveredUnit> {
        // Try exact match on qualified name first (highest priority)
        if let Some(unit) = self.get_unit_by_qualified_name(name_or_path) {
            return vec![unit];
        }

        // Try matching qualified name with partial path (ends_with for suffix matching)
        // Must end with the path and either be at the start or preceded by a /
        let suffix_matches: Vec<_> = self
            .units_by_qualified_name
            .iter()
            .filter(|(qname, _)| {
                qname.ends_with(name_or_path)
                    && (qname.len() == name_or_path.len()
                        || qname
                            .chars()
                            .nth(qname.len() - name_or_path.len() - 1)
                            == Some('/'))
            })
            .map(|(_, unit)| unit)
            .collect();

        if !suffix_matches.is_empty() {
            return suffix_matches;
        }

        // Try path prefix match - return all units under this path
        let prefix_matches = self.get_units_by_path_prefix(name_or_path);
        if !prefix_matches.is_empty() {
            return prefix_matches;
        }

        // Fall back to simple name match (may return multiple)
        self.get_units_by_name(name_or_path)
    }

    /// Get all units that are descendants of a given path
    /// E.g., "infrastructure" returns all units under infrastructure/*
    /// E.g., "infrastructure/database" returns all units under infrastructure/database/*
    pub fn get_units_by_path_prefix(&self, path_prefix: &str) -> Vec<&DiscoveredUnit> {
        let normalized_prefix = path_prefix.trim_end_matches('/');

        self.units_by_qualified_name
            .values()
            .filter(|unit| {
                let unit_path = unit.qualified_name.as_str();
                // Match if unit is under this path (but not exactly this path, as that's handled separately)
                unit_path.starts_with(normalized_prefix)
                    && unit_path.len() > normalized_prefix.len()
                    && unit_path.chars().nth(normalized_prefix.len()) == Some('/')
            })
            .collect()
    }

    /// Check if a simple name is ambiguous (has duplicates)
    pub fn is_ambiguous(&self, name: &str) -> bool {
        self.units
            .get(name)
            .map(|units| units.len() > 1)
            .unwrap_or(false)
    }

    /// Get all duplicate unit names in the registry
    pub fn get_duplicate_names(&self) -> Vec<String> {
        self.units
            .iter()
            .filter_map(|(name, units)| {
                if units.len() > 1 {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}
