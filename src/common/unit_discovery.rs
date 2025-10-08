use crate::common::Result;
use crate::common::unit_config::{UnitConfig, DiscoveredUnit, UnitRegistry, UnitType};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Discovers all units in a project by recursively finding .envie files
pub struct UnitDiscovery {
    pub root_path: PathBuf,
    pub registry: UnitRegistry,
}

impl UnitDiscovery {
    pub fn new(root_path: PathBuf) -> Self {
        Self {
            root_path,
            registry: UnitRegistry::new(),
        }
    }
    
    /// Discover all units in the project
    pub fn discover_all(&mut self) -> Result<()> {
        self.registry = UnitRegistry::new();
        
        // Find all .envie files recursively
        for entry in WalkDir::new(&self.root_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name() == ".envie")
        {
            let path = entry.path();
            let unit = self.parse_unit_file(path)?;
            self.registry.add_unit(unit);
        }
        
        // Build parent-child relationships
        self.build_hierarchy()?;
        
        Ok(())
    }
    
    /// Parse a single .envie file
    fn parse_unit_file(&self, file_path: &Path) -> Result<DiscoveredUnit> {
        let content = fs::read_to_string(file_path)?;
        let config: UnitConfig = serde_yaml::from_str(&content)?;
        
        // Calculate the level (depth from root)
        let level = self.calculate_level(file_path);
        
        // Set the path relative to project root
        let relative_path = file_path.parent()
            .unwrap_or(file_path)
            .strip_prefix(&self.root_path)
            .unwrap_or(file_path.parent().unwrap_or(file_path))
            .to_path_buf();
        
        let mut unit = DiscoveredUnit::new(config, relative_path.clone(), level);
        
        // Set the path in the config
        unit.config.path = relative_path.to_string_lossy().to_string();
        
        Ok(unit)
    }
    
    /// Calculate the depth level of a unit
    fn calculate_level(&self, file_path: &Path) -> usize {
        file_path
            .strip_prefix(&self.root_path)
            .unwrap_or(file_path)
            .components()
            .count()
            .saturating_sub(2) // Subtract 2: 1 for the .envie file, 1 for the directory containing it
    }
    
    /// Build parent-child relationships between units
    fn build_hierarchy(&mut self) -> Result<()> {
        let unit_data: Vec<(String, String, PathBuf, usize)> = self.registry.units_by_qualified_name
            .iter()
            .map(|(qname, unit)| (qname.clone(), unit.config.name.clone(), unit.path.clone(), unit.level))
            .collect();

        for (qualified_name, unit_name, unit_path, unit_level) in unit_data {
            // Find parent (closest ancestor with .envie file)
            if let Some(parent) = self.find_parent(&unit_path, unit_level) {
                // Find parent unit name
                if let Some(parent_unit) = self.registry.get_unit_by_path(&parent) {
                    let parent_qualified_name = parent_unit.qualified_name.clone();

                    // Add this unit as a child of the parent
                    if let Some(parent_unit_mut) = self.registry.units_by_qualified_name.get_mut(&parent_qualified_name) {
                        parent_unit_mut.children.push(unit_path.clone());
                    }
                }

                // Set parent reference
                if let Some(unit_mut) = self.registry.units_by_qualified_name.get_mut(&qualified_name) {
                    unit_mut.parent = Some(parent);
                }
            }
        }

        Ok(())
    }
    
    /// Find the parent unit for a given unit
    fn find_parent(&self, unit_path: &PathBuf, unit_level: usize) -> Option<PathBuf> {
        if unit_level == 0 {
            return None; // Root level units have no parent
        }
        
        // Look for parent directories that contain .envie files
        let mut current_path = unit_path.clone();
        
        while let Some(parent) = current_path.parent() {
            if parent == self.root_path {
                break; // Reached project root
            }
            
            let parent_envie = parent.join(".envie");
            if parent_envie.exists() {
                return Some(parent.to_path_buf());
            }
            
            current_path = parent.to_path_buf();
        }
        
        None
    }
    
    /// Get all units of a specific type
    pub fn get_units_by_type(&self, unit_type: &UnitType) -> Vec<&DiscoveredUnit> {
        self.registry.get_units_by_type(unit_type)
    }
    
    /// Get all root-level units (units at depth 0)
    pub fn get_root_units(&self) -> Vec<&DiscoveredUnit> {
        self.registry
            .get_all_units()
            .into_iter()
            .filter(|unit| unit.is_root_level())
            .collect()
    }
    
    /// Get all leaf units (units with no children)
    pub fn get_leaf_units(&self) -> Vec<&DiscoveredUnit> {
        self.registry
            .get_all_units()
            .into_iter()
            .filter(|unit| unit.is_leaf())
            .collect()
    }

    /// Get all units in the registry
    pub fn get_all_units(&self) -> Vec<&DiscoveredUnit> {
        self.registry.get_all_units()
    }
    
    /// Find units by name pattern
    pub fn find_units_by_name(&self, pattern: &str) -> Vec<&DiscoveredUnit> {
        self.registry.find_units_by_pattern(pattern)
    }
    
    /// Get the dependency graph for a unit
    pub fn get_dependency_graph(&self, unit_name: &str) -> Result<Vec<&DiscoveredUnit>> {
        let mut visited = std::collections::HashSet::new();
        let mut result = Vec::new();
        
        self.collect_dependencies(unit_name, &mut visited, &mut result)?;
        
        Ok(result)
    }
    
    /// Recursively collect dependencies
    fn collect_dependencies<'a>(
        &'a self,
        unit_name: &str,
        visited: &mut std::collections::HashSet<String>,
        result: &mut Vec<&'a DiscoveredUnit>,
    ) -> Result<()> {
        if visited.contains(unit_name) {
            return Ok(()); // Avoid circular dependencies
        }
        
        visited.insert(unit_name.to_string());
        
        if let Some(unit) = self.registry.get_unit(unit_name) {
            for dep in &unit.config.depends {
                // Resolve the dependency path to a unit name
                if let Some(dep_unit) = self.resolve_dependency_path(&unit.path, &dep.path) {
                    self.collect_dependencies(&dep_unit, visited, result)?;
                    if let Some(dep_unit_ref) = self.registry.get_unit(&dep_unit) {
                        result.push(dep_unit_ref);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Resolve a dependency path to a unit name
    fn resolve_dependency_path(&self, from_path: &PathBuf, dep_path: &str) -> Option<String> {
        // Convert relative path to path relative to project root
        let from_dir = from_path;
        let dep_relative = from_dir.join(dep_path);

        // Normalize the path (resolve ../ and ./)
        let dep_normalized = self.normalize_path(&dep_relative);

        // Find the unit by path
        if let Some(unit) = self.registry.get_unit_by_path(&dep_normalized) {
            return Some(unit.config.name.clone());
        }

        None
    }

    /// Normalize a path by resolving . and .. components
    fn normalize_path(&self, path: &PathBuf) -> PathBuf {
        let mut components = Vec::new();

        for component in path.components() {
            match component {
                std::path::Component::ParentDir => {
                    components.pop();
                }
                std::path::Component::CurDir => {
                    // Skip current directory
                }
                _ => {
                    components.push(component);
                }
            }
        }

        components.iter().collect()
    }
    
    /// Get all units in dependency order (topological sort)
    pub fn get_units_in_dependency_order(&self) -> Result<Vec<&DiscoveredUnit>> {
        let mut visiting = std::collections::HashSet::new();
        let mut visited = std::collections::HashSet::new();
        let mut result = Vec::new();

        // Process ALL units in dependency order using topological sort
        for unit in self.registry.get_all_units() {
            if !visited.contains(&unit.config.name) {
                self.topological_visit(&unit.config.name, &mut visiting, &mut visited, &mut result)?;
            }
        }

        Ok(result)
    }

    /// Topological sort helper - visits unit after all its dependencies
    fn topological_visit<'a>(
        &'a self,
        unit_name: &str,
        visiting: &mut std::collections::HashSet<String>,
        visited: &mut std::collections::HashSet<String>,
        result: &mut Vec<&'a DiscoveredUnit>,
    ) -> Result<()> {
        // Check for cycles
        if visiting.contains(unit_name) {
            return Err(crate::common::EnvieError::ValidationError(
                format!("Circular dependency detected involving unit: {}", unit_name)
            ));
        }

        // Already fully processed
        if visited.contains(unit_name) {
            return Ok(());
        }

        if let Some(unit) = self.registry.get_unit(unit_name) {
            // Mark as currently visiting (in recursion stack)
            visiting.insert(unit_name.to_string());

            // First, recursively visit all dependencies
            for dep in &unit.config.depends {
                if let Some(dep_unit) = self.resolve_dependency_path(&unit.path, &dep.path) {
                    self.topological_visit(&dep_unit, visiting, visited, result)?;
                }
            }

            // Remove from visiting, mark as fully processed
            visiting.remove(unit_name);
            visited.insert(unit_name.to_string());

            // Then add this unit to the result (after all dependencies)
            result.push(unit);
        }

        Ok(())
    }
    
    /// Resolve deployment order for a specific unit
    pub fn resolve_deployment_order(&self, unit_name: &str) -> Result<Vec<&DiscoveredUnit>> {
        let mut visited = std::collections::HashSet::new();
        let mut result = Vec::new();
        
        // Collect all dependencies recursively
        self.collect_dependencies(unit_name, &mut visited, &mut result)?;
        
        // Add the unit itself at the end
        if let Some(unit) = self.registry.get_unit(unit_name) {
            result.push(unit);
        }
        
        Ok(result)
    }
}

/// Helper function to create a new unit discovery
pub fn discover_units(root_path: PathBuf) -> Result<UnitRegistry> {
    let mut discovery = UnitDiscovery::new(root_path);
    discovery.discover_all()?;
    Ok(discovery.registry)
}
