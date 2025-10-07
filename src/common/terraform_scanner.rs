use crate::common::*;
use std::path::Path;
use std::collections::HashSet;
use regex::Regex;

#[derive(Debug, Clone)]
pub struct TerraformDependency {
    pub data_source_name: String,
    pub backend_type: String,
    pub backend_config: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct TerraformScanner {
    // Cache for compiled regex patterns
    data_source_pattern: Regex,
    backend_pattern: Regex,
}

impl TerraformScanner {
    pub fn new() -> Result<Self> {
        Ok(Self {
            data_source_pattern: Regex::new(r#"data\s+"terraform_remote_state"\s+"([^"]+)""#)?,
            backend_pattern: Regex::new(r#"backend\s*=\s*"([^"]+)""#)?,
        })
    }

    /// Scan a Terraform file and extract all terraform_remote_state data sources
    pub fn scan_file<P: AsRef<Path>>(&self, file_path: P) -> Result<Vec<TerraformDependency>> {
        let content = std::fs::read_to_string(file_path)?;
        self.scan_content(&content)
    }

    /// Scan Terraform content and extract data sources
    pub fn scan_content(&self, content: &str) -> Result<Vec<TerraformDependency>> {
        let mut dependencies = Vec::new();
        let mut current_data_source: Option<String> = None;
        let mut current_backend: Option<String> = None;
        let mut current_config: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let mut in_data_source = false;
        let mut in_backend_config = false;

        for line in content.lines() {
            let line = line.trim();

            // Check for data source start
            if let Some(caps) = self.data_source_pattern.captures(line) {
                if let Some(prev) = current_data_source.take() {
                    // Save previous data source
                    dependencies.push(TerraformDependency {
                        data_source_name: prev,
                        backend_type: current_backend.unwrap_or_else(|| "s3".to_string()),
                        backend_config: current_config.clone(),
                    });
                }
                current_data_source = Some(caps[1].to_string());
                current_backend = None;
                current_config.clear();
                in_data_source = true;
                in_backend_config = false;
                continue;
            }

            if in_data_source {
                // Check for backend type
                if let Some(caps) = self.backend_pattern.captures(line) {
                    current_backend = Some(caps[1].to_string());
                    in_backend_config = true;
                    continue;
                }

                // Check for config block
                if line.contains("config") && line.contains("{") {
                    in_backend_config = true;
                    continue;
                }

                // Parse config values
                if in_backend_config && line.contains("=") {
                    if let Some((key, value)) = self.parse_config_line(line) {
                        current_config.insert(key, value);
                    }
                    continue;
                }

                // Check for end of data source
                if line == "}" && !in_backend_config {
                    if let Some(data_source_name) = current_data_source.take() {
                        dependencies.push(TerraformDependency {
                            data_source_name,
                            backend_type: current_backend.unwrap_or_else(|| "s3".to_string()),
                            backend_config: current_config.clone(),
                        });
                    }
                    in_data_source = false;
                    in_backend_config = false;
                    current_backend = None;
                    current_config.clear();
                }
            }
        }

        // Handle case where file ends without closing brace
        if let Some(data_source_name) = current_data_source.take() {
            dependencies.push(TerraformDependency {
                data_source_name,
                backend_type: current_backend.unwrap_or_else(|| "s3".to_string()),
                backend_config: current_config.clone(),
            });
        }

        Ok(dependencies)
    }

    /// Scan all Terraform files in a directory
    pub fn scan_directory<P: AsRef<Path>>(&self, dir_path: P) -> Result<Vec<TerraformDependency>> {
        let mut all_dependencies = Vec::new();
        
        for entry in std::fs::read_dir(dir_path)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() && path.extension().map_or(false, |ext| ext == "tf") {
                let deps = self.scan_file(&path)?;
                all_dependencies.extend(deps);
            }
        }
        
        Ok(all_dependencies)
    }

    /// Parse a config line like 'bucket = "my-bucket"'
    fn parse_config_line(&self, line: &str) -> Option<(String, String)> {
        let parts: Vec<&str> = line.split('=').collect();
        if parts.len() == 2 {
            let key = parts[0].trim().to_string();
            let value = parts[1].trim().trim_matches('"').to_string();
            Some((key, value))
        } else {
            None
        }
    }

    /// Extract used outputs from Terraform content
    pub fn extract_used_outputs(&self, content: &str, data_source_name: &str) -> HashSet<String> {
        let mut used_outputs = HashSet::new();
        let pattern = format!(r#"data\.terraform_remote_state\.{}\.outputs\.(\w+)"#, data_source_name);
        
        if let Ok(regex) = Regex::new(&pattern) {
            for caps in regex.captures_iter(content) {
                if let Some(output) = caps.get(1) {
                    used_outputs.insert(output.as_str().to_string());
                }
            }
        }
        
        used_outputs
    }
}

impl Default for TerraformScanner {
    fn default() -> Self {
        Self::new().expect("Failed to create TerraformScanner")
    }
}
