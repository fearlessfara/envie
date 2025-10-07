use crate::common::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerraformOutput {
    pub value: serde_json::Value,
    #[serde(rename = "type")]
    pub output_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerraformState {
    pub service: String,
    pub dependencies: Vec<String>,
}

pub struct TerraformManager {
    working_directory: std::path::PathBuf,
    verbose: bool,
}

impl TerraformManager {
    pub fn new<P: AsRef<Path>>(working_directory: P) -> Self {
        Self {
            working_directory: working_directory.as_ref().to_path_buf(),
            verbose: false,
        }
    }

    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    pub fn init(&self) -> Result<()> {
        self.run_command("init", &[], false)
    }

    pub fn init_with_upgrade(&self) -> Result<()> {
        self.run_command("init", &["-upgrade"], false)
    }

    pub fn workspace_list(&self) -> Result<Vec<String>> {
        let output = self.run_command_capture("workspace", &["list"], false)?;
        let workspaces: Vec<String> = output
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();
        Ok(workspaces)
    }

    pub fn workspace_show(&self) -> Result<String> {
        let output = self.run_command_capture("workspace", &["show"], false)?;
        Ok(output.trim().to_string())
    }

    pub fn workspace_select(&self, workspace: &str) -> Result<()> {
        self.run_command("workspace", &["select", workspace], false)
    }

    pub fn workspace_new(&self, workspace: &str) -> Result<()> {
        self.run_command("workspace", &["new", workspace], false)
    }

    pub fn workspace_delete(&self, workspace: &str) -> Result<()> {
        self.run_command("workspace", &["delete", workspace], false)
    }

    pub fn apply(&self, vars: &[(&str, &str)]) -> Result<()> {
        let mut args = vec!["-auto-approve", "-input=false"];
        let mut var_args = Vec::new();
        for (key, value) in vars {
            let var_arg = format!("{}={}", key, value);
            var_args.push(var_arg);
        }
        
        for var_arg in &var_args {
            args.extend(&["-var", var_arg]);
        }
        self.run_command("apply", &args, false)
    }

    pub fn apply_with_output(&self, vars: &[(&str, &str)], output_file: &str) -> Result<()> {
        let mut args = vec!["-auto-approve", "-input=false"];
        let mut var_args = Vec::new();
        for (key, value) in vars {
            let var_arg = format!("{}={}", key, value);
            var_args.push(var_arg);
        }
        
        for var_arg in &var_args {
            args.extend(&["-var", var_arg]);
        }
        args.extend(&["-out", output_file]);
        self.run_command("apply", &args, false)
    }

    pub fn destroy(&self, vars: &[(&str, &str)]) -> Result<()> {
        let mut args = vec!["-auto-approve", "-input=false"];
        let mut var_args = Vec::new();
        for (key, value) in vars {
            let var_arg = format!("{}={}", key, value);
            var_args.push(var_arg);
        }
        
        for var_arg in &var_args {
            args.extend(&["-var", var_arg]);
        }
        self.run_command("destroy", &args, false)
    }

    pub fn output_json(&self) -> Result<HashMap<String, TerraformOutput>> {
        let output = self.run_command_capture("output", &["-json"], false)?;
        let parsed: HashMap<String, TerraformOutput> = serde_json::from_str(&output)?;
        Ok(parsed)
    }

    pub fn output_value(&self, key: &str) -> Result<serde_json::Value> {
        let output = self.run_command_capture("output", &["-json", key], false)?;
        let parsed: serde_json::Value = serde_json::from_str(&output)?;
        Ok(parsed)
    }

    fn run_command(&self, command: &str, args: &[&str], _quiet: bool) -> Result<()> {
        let mut cmd = Command::new("terraform");
        cmd.arg(command);
        cmd.args(args);
        cmd.current_dir(&self.working_directory);
        
        // Set GODEBUG environment variable as in the original scripts
        cmd.env("GODEBUG", "asyncpreemptoff=1");

        if self.verbose {
            println!(">> Running: terraform {} {}", command, args.join(" "));
        }

        let output = cmd.output();

        match output {
            Ok(output) => {
                if output.status.success() {
                    Ok(())
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Err(crate::common::EnvieError::TerraformError(
                        format!("terraform {} failed: {}", command, stderr)
                    ))
                }
            }
            Err(e) => Err(crate::common::EnvieError::ProcessError(
                format!("Failed to execute terraform {}: {}", command, e)
            )),
        }
    }

    fn run_command_capture(&self, command: &str, args: &[&str], _quiet: bool) -> Result<String> {
        let mut cmd = Command::new("terraform");
        cmd.arg(command);
        cmd.args(args);
        cmd.current_dir(&self.working_directory);
        
        // Set GODEBUG environment variable as in the original scripts
        cmd.env("GODEBUG", "asyncpreemptoff=1");

        if self.verbose {
            println!(">> Running: terraform {} {}", command, args.join(" "));
        }

        let output = cmd.output()?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(crate::common::EnvieError::TerraformError(
                format!("terraform {} failed: {}", command, stderr)
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_terraform_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = TerraformManager::new(temp_dir.path());
        assert_eq!(manager.working_directory, temp_dir.path());
        assert!(!manager.verbose);
    }

    #[test]
    fn test_terraform_manager_with_verbose() {
        let temp_dir = TempDir::new().unwrap();
        let manager = TerraformManager::new(temp_dir.path()).with_verbose(true);
        assert!(manager.verbose);
    }
}
