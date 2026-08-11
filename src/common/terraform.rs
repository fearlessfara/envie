use crate::common::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerraformOutput {
    pub value: serde_json::Value,
    /// Terraform's own type description, which is a bare string only for
    /// primitives — a list output reports `["list", "string"]`, and an object
    /// reports a nested structure.
    #[serde(rename = "type")]
    pub output_type: serde_json::Value,
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

    /// Initialise with an explicit backend configuration.
    ///
    /// `-reconfigure` is always passed: Envie points the same directory at a
    /// different state path for every environment, and without it Terraform
    /// refuses to continue once the recorded backend differs. Reconfiguring
    /// discards that record rather than migrating state, which is what Envie
    /// wants, since each environment has its own state.
    pub fn init_with_backend_config(&self, backend_config: &[(&str, &str)]) -> Result<()> {
        let mut args = vec!["-reconfigure".to_string(), "-input=false".to_string()];
        for (key, value) in backend_config {
            args.push(format!("-backend-config={}={}", key, value));
        }
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        self.run_command("init", &arg_refs, false)
    }

    pub fn workspace_list(&self) -> Result<Vec<String>> {
        let output = self.run_command_capture("workspace", &["list"], false)?;
        let workspaces: Vec<String> = output
            .lines()
            .map(|line| {
                // Remove the * prefix and trim whitespace
                line.trim().trim_start_matches("* ").to_string()
            })
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
        self.apply_with_var_files(vars, &[])
    }

    /// Apply with variables and `-var-file` arguments.
    ///
    /// Var files are relative to the unit directory, and missing ones are
    /// skipped: an environment may declare a file that only some units have.
    pub fn apply_with_var_files(&self, vars: &[(&str, &str)], var_files: &[String]) -> Result<()> {
        let mut args = vec!["-auto-approve".to_string(), "-input=false".to_string()];
        args.extend(self.variable_arguments(vars, var_files));
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        self.run_command("apply", &arg_refs, false)
    }

    pub fn plan(&self, vars: &[(&str, &str)], var_files: &[String]) -> Result<()> {
        let mut args = vec!["-input=false".to_string()];
        args.extend(self.variable_arguments(vars, var_files));
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        self.run_command("plan", &arg_refs, false)
    }

    fn variable_arguments(&self, vars: &[(&str, &str)], var_files: &[String]) -> Vec<String> {
        let mut args = Vec::new();
        for file in var_files {
            if self.working_directory.join(file).exists() {
                args.push(format!("-var-file={}", file));
            }
        }
        for (key, value) in vars {
            args.push("-var".to_string());
            args.push(format!("{}={}", key, value));
        }
        args
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
        self.destroy_with_var_files(vars, &[])
    }

    pub fn destroy_with_var_files(
        &self,
        vars: &[(&str, &str)],
        var_files: &[String],
    ) -> Result<()> {
        let mut args = vec!["-auto-approve".to_string(), "-input=false".to_string()];
        args.extend(self.variable_arguments(vars, var_files));
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        self.run_command("destroy", &arg_refs, false)
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
            // In verbose mode, inherit stdout/stderr to show terraform output
            let status = cmd.status();
            match status {
                Ok(status) => {
                    if status.success() {
                        Ok(())
                    } else {
                        Err(crate::common::EnvieError::TerraformError(format!(
                            "terraform {} failed with exit code: {}",
                            command, status
                        )))
                    }
                }
                Err(e) => Err(crate::common::EnvieError::ProcessError(format!(
                    "Failed to execute terraform {}: {}",
                    command, e
                ))),
            }
        } else {
            // In non-verbose mode, capture output
            let output = cmd.output();
            match output {
                Ok(output) => {
                    if output.status.success() {
                        Ok(())
                    } else {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        Err(crate::common::EnvieError::TerraformError(format!(
                            "terraform {} failed: {}",
                            command, stderr
                        )))
                    }
                }
                Err(e) => Err(crate::common::EnvieError::ProcessError(format!(
                    "Failed to execute terraform {}: {}",
                    command, e
                ))),
            }
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
            Err(crate::common::EnvieError::TerraformError(format!(
                "terraform {} failed: {}",
                command, stderr
            )))
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
