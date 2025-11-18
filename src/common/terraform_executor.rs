use crate::common::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;

/// Trait for executing Terraform commands
/// This allows us to inject mock implementations for testing
#[async_trait]
pub trait TerraformExecutor: Send + Sync {
    /// Initialize Terraform in the given directory
    async fn init(&self, working_dir: &Path, upgrade: bool) -> Result<String>;

    /// Run terraform plan
    async fn plan(&self, working_dir: &Path, vars: &[(&str, &str)]) -> Result<String>;

    /// Run terraform apply
    async fn apply(&self, working_dir: &Path, vars: &[(&str, &str)]) -> Result<String>;

    /// Run terraform destroy
    async fn destroy(&self, working_dir: &Path, vars: &[(&str, &str)]) -> Result<String>;

    /// Get terraform output
    async fn output(&self, working_dir: &Path) -> Result<HashMap<String, serde_json::Value>>;

    /// List workspaces
    async fn workspace_list(&self, working_dir: &Path) -> Result<Vec<String>>;

    /// Select workspace
    async fn workspace_select(&self, working_dir: &Path, workspace: &str) -> Result<String>;

    /// Create workspace
    async fn workspace_new(&self, working_dir: &Path, workspace: &str) -> Result<String>;

    /// Show current workspace
    async fn workspace_show(&self, working_dir: &Path) -> Result<String>;

    /// Validate configuration
    async fn validate(&self, working_dir: &Path) -> Result<String>;

    /// Get version
    async fn version(&self) -> Result<String>;
}

/// Real Terraform executor that calls actual terraform CLI
pub struct RealTerraformExecutor;

#[async_trait]
impl TerraformExecutor for RealTerraformExecutor {
    async fn init(&self, working_dir: &Path, upgrade: bool) -> Result<String> {
        let mut cmd = tokio::process::Command::new("terraform");
        cmd.arg("init")
            .current_dir(working_dir);

        if upgrade {
            cmd.arg("-upgrade");
        }

        let output = cmd.output().await?;
        if !output.status.success() {
            return Err(crate::common::EnvieError::TerraformError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn plan(&self, working_dir: &Path, vars: &[(&str, &str)]) -> Result<String> {
        let mut cmd = tokio::process::Command::new("terraform");
        cmd.arg("plan")
            .current_dir(working_dir);

        for (key, value) in vars {
            cmd.arg("-var").arg(format!("{}={}", key, value));
        }

        let output = cmd.output().await?;
        if !output.status.success() {
            return Err(crate::common::EnvieError::TerraformError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn apply(&self, working_dir: &Path, vars: &[(&str, &str)]) -> Result<String> {
        let mut cmd = tokio::process::Command::new("terraform");
        cmd.arg("apply")
            .arg("-auto-approve")
            .current_dir(working_dir);

        for (key, value) in vars {
            cmd.arg("-var").arg(format!("{}={}", key, value));
        }

        let output = cmd.output().await?;
        if !output.status.success() {
            return Err(crate::common::EnvieError::TerraformError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn destroy(&self, working_dir: &Path, vars: &[(&str, &str)]) -> Result<String> {
        let mut cmd = tokio::process::Command::new("terraform");
        cmd.arg("destroy")
            .arg("-auto-approve")
            .current_dir(working_dir);

        for (key, value) in vars {
            cmd.arg("-var").arg(format!("{}={}", key, value));
        }

        let output = cmd.output().await?;
        if !output.status.success() {
            return Err(crate::common::EnvieError::TerraformError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn output(&self, working_dir: &Path) -> Result<HashMap<String, serde_json::Value>> {
        let cmd = tokio::process::Command::new("terraform")
            .arg("output")
            .arg("-json")
            .current_dir(working_dir)
            .output()
            .await?;

        if !cmd.status.success() {
            return Err(crate::common::EnvieError::TerraformError(
                String::from_utf8_lossy(&cmd.stderr).to_string(),
            ));
        }

        let output_str = String::from_utf8_lossy(&cmd.stdout);
        let outputs: HashMap<String, serde_json::Value> = serde_json::from_str(&output_str)?;
        Ok(outputs)
    }

    async fn workspace_list(&self, working_dir: &Path) -> Result<Vec<String>> {
        let output = tokio::process::Command::new("terraform")
            .arg("workspace")
            .arg("list")
            .current_dir(working_dir)
            .output()
            .await?;

        if !output.status.success() {
            return Err(crate::common::EnvieError::TerraformError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let workspaces: Vec<String> = output_str
            .lines()
            .map(|line| line.trim().trim_start_matches("* ").to_string())
            .filter(|line| !line.is_empty())
            .collect();

        Ok(workspaces)
    }

    async fn workspace_select(&self, working_dir: &Path, workspace: &str) -> Result<String> {
        let output = tokio::process::Command::new("terraform")
            .arg("workspace")
            .arg("select")
            .arg(workspace)
            .current_dir(working_dir)
            .output()
            .await?;

        if !output.status.success() {
            return Err(crate::common::EnvieError::TerraformError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn workspace_new(&self, working_dir: &Path, workspace: &str) -> Result<String> {
        let output = tokio::process::Command::new("terraform")
            .arg("workspace")
            .arg("new")
            .arg(workspace)
            .current_dir(working_dir)
            .output()
            .await?;

        if !output.status.success() {
            return Err(crate::common::EnvieError::TerraformError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn workspace_show(&self, working_dir: &Path) -> Result<String> {
        let output = tokio::process::Command::new("terraform")
            .arg("workspace")
            .arg("show")
            .current_dir(working_dir)
            .output()
            .await?;

        if !output.status.success() {
            return Err(crate::common::EnvieError::TerraformError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    async fn validate(&self, working_dir: &Path) -> Result<String> {
        let output = tokio::process::Command::new("terraform")
            .arg("validate")
            .current_dir(working_dir)
            .output()
            .await?;

        if !output.status.success() {
            return Err(crate::common::EnvieError::TerraformError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn version(&self) -> Result<String> {
        let output = tokio::process::Command::new("terraform")
            .arg("--version")
            .output()
            .await?;

        if !output.status.success() {
            return Err(crate::common::EnvieError::TerraformError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[cfg(test)]
pub mod test_utils {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// Mock Terraform executor for testing
    #[derive(Clone)]
    pub struct MockTerraformExecutor {
        pub init_calls: Arc<Mutex<Vec<(String, bool)>>>,
        pub plan_calls: Arc<Mutex<Vec<String>>>,
        pub apply_calls: Arc<Mutex<Vec<String>>>,
        pub destroy_calls: Arc<Mutex<Vec<String>>>,
        pub workspace_list_result: Arc<Mutex<Vec<String>>>,
        pub workspace_show_result: Arc<Mutex<String>>,
        pub output_result: Arc<Mutex<HashMap<String, serde_json::Value>>>,
        pub should_fail: Arc<Mutex<bool>>,
    }

    impl Default for MockTerraformExecutor {
        fn default() -> Self {
            Self {
                init_calls: Arc::new(Mutex::new(Vec::new())),
                plan_calls: Arc::new(Mutex::new(Vec::new())),
                apply_calls: Arc::new(Mutex::new(Vec::new())),
                destroy_calls: Arc::new(Mutex::new(Vec::new())),
                workspace_list_result: Arc::new(Mutex::new(vec!["default".to_string()])),
                workspace_show_result: Arc::new(Mutex::new("default".to_string())),
                output_result: Arc::new(Mutex::new(HashMap::new())),
                should_fail: Arc::new(Mutex::new(false)),
            }
        }
    }

    impl MockTerraformExecutor {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn with_workspaces(mut self, workspaces: Vec<String>) -> Self {
            *self.workspace_list_result.lock().unwrap() = workspaces;
            self
        }

        pub fn with_output(mut self, output: HashMap<String, serde_json::Value>) -> Self {
            *self.output_result.lock().unwrap() = output;
            self
        }

        pub fn set_should_fail(&self, should_fail: bool) {
            *self.should_fail.lock().unwrap() = should_fail;
        }

        pub fn get_init_calls(&self) -> Vec<(String, bool)> {
            self.init_calls.lock().unwrap().clone()
        }

        pub fn get_apply_calls(&self) -> Vec<String> {
            self.apply_calls.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl TerraformExecutor for MockTerraformExecutor {
        async fn init(&self, working_dir: &Path, upgrade: bool) -> Result<String> {
            if *self.should_fail.lock().unwrap() {
                return Err(crate::common::EnvieError::TerraformError(
                    "Mock init failed".to_string(),
                ));
            }
            self.init_calls
                .lock()
                .unwrap()
                .push((working_dir.display().to_string(), upgrade));
            Ok("Terraform initialized (mock)".to_string())
        }

        async fn plan(&self, working_dir: &Path, _vars: &[(&str, &str)]) -> Result<String> {
            if *self.should_fail.lock().unwrap() {
                return Err(crate::common::EnvieError::TerraformError(
                    "Mock plan failed".to_string(),
                ));
            }
            self.plan_calls
                .lock()
                .unwrap()
                .push(working_dir.display().to_string());
            Ok("Plan: 1 to add, 0 to change, 0 to destroy (mock)".to_string())
        }

        async fn apply(&self, working_dir: &Path, _vars: &[(&str, &str)]) -> Result<String> {
            if *self.should_fail.lock().unwrap() {
                return Err(crate::common::EnvieError::TerraformError(
                    "Mock apply failed".to_string(),
                ));
            }
            self.apply_calls
                .lock()
                .unwrap()
                .push(working_dir.display().to_string());
            Ok("Apply complete! Resources: 1 added (mock)".to_string())
        }

        async fn destroy(&self, working_dir: &Path, _vars: &[(&str, &str)]) -> Result<String> {
            if *self.should_fail.lock().unwrap() {
                return Err(crate::common::EnvieError::TerraformError(
                    "Mock destroy failed".to_string(),
                ));
            }
            self.destroy_calls
                .lock()
                .unwrap()
                .push(working_dir.display().to_string());
            Ok("Destroy complete! Resources: 1 destroyed (mock)".to_string())
        }

        async fn output(&self, _working_dir: &Path) -> Result<HashMap<String, serde_json::Value>> {
            if *self.should_fail.lock().unwrap() {
                return Err(crate::common::EnvieError::TerraformError(
                    "Mock output failed".to_string(),
                ));
            }
            Ok(self.output_result.lock().unwrap().clone())
        }

        async fn workspace_list(&self, _working_dir: &Path) -> Result<Vec<String>> {
            if *self.should_fail.lock().unwrap() {
                return Err(crate::common::EnvieError::TerraformError(
                    "Mock workspace list failed".to_string(),
                ));
            }
            Ok(self.workspace_list_result.lock().unwrap().clone())
        }

        async fn workspace_select(&self, _working_dir: &Path, workspace: &str) -> Result<String> {
            if *self.should_fail.lock().unwrap() {
                return Err(crate::common::EnvieError::TerraformError(
                    "Mock workspace select failed".to_string(),
                ));
            }
            *self.workspace_show_result.lock().unwrap() = workspace.to_string();
            Ok(format!("Switched to workspace \"{}\" (mock)", workspace))
        }

        async fn workspace_new(&self, _working_dir: &Path, workspace: &str) -> Result<String> {
            if *self.should_fail.lock().unwrap() {
                return Err(crate::common::EnvieError::TerraformError(
                    "Mock workspace new failed".to_string(),
                ));
            }
            let mut list = self.workspace_list_result.lock().unwrap();
            if !list.contains(&workspace.to_string()) {
                list.push(workspace.to_string());
            }
            Ok(format!("Created and switched to workspace \"{}\" (mock)", workspace))
        }

        async fn workspace_show(&self, _working_dir: &Path) -> Result<String> {
            if *self.should_fail.lock().unwrap() {
                return Err(crate::common::EnvieError::TerraformError(
                    "Mock workspace show failed".to_string(),
                ));
            }
            Ok(self.workspace_show_result.lock().unwrap().clone())
        }

        async fn validate(&self, _working_dir: &Path) -> Result<String> {
            if *self.should_fail.lock().unwrap() {
                return Err(crate::common::EnvieError::TerraformError(
                    "Mock validate failed".to_string(),
                ));
            }
            Ok("Success! The configuration is valid (mock)".to_string())
        }

        async fn version(&self) -> Result<String> {
            if *self.should_fail.lock().unwrap() {
                return Err(crate::common::EnvieError::TerraformError(
                    "Mock version failed".to_string(),
                ));
            }
            Ok("Terraform v1.5.0 (mock)".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_utils::*;
    use super::*;

    #[tokio::test]
    async fn test_mock_executor_init() {
        let executor = MockTerraformExecutor::new();
        let result = executor.init(Path::new("/tmp/test"), false).await;

        assert!(result.is_ok());
        assert_eq!(executor.get_init_calls().len(), 1);
        assert_eq!(executor.get_init_calls()[0].0, "/tmp/test");
        assert_eq!(executor.get_init_calls()[0].1, false);
    }

    #[tokio::test]
    async fn test_mock_executor_apply() {
        let executor = MockTerraformExecutor::new();
        let result = executor.apply(Path::new("/tmp/test"), &[]).await;

        assert!(result.is_ok());
        assert_eq!(executor.get_apply_calls().len(), 1);
        assert_eq!(executor.get_apply_calls()[0], "/tmp/test");
    }

    #[tokio::test]
    async fn test_mock_executor_failure() {
        let executor = MockTerraformExecutor::new();
        executor.set_should_fail(true);

        let result = executor.init(Path::new("/tmp/test"), false).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_executor_workspaces() {
        let executor = MockTerraformExecutor::new()
            .with_workspaces(vec![
                "default".to_string(),
                "dev-123".to_string(),
            ]);

        let workspaces = executor.workspace_list(Path::new("/tmp/test")).await.unwrap();
        assert_eq!(workspaces.len(), 2);
        assert!(workspaces.contains(&"dev-123".to_string()));
    }

    #[tokio::test]
    async fn test_mock_executor_output() {
        let mut output_data = HashMap::new();
        output_data.insert("vpc_id".to_string(), serde_json::json!("vpc-123"));

        let executor = MockTerraformExecutor::new().with_output(output_data);

        let output = executor.output(Path::new("/tmp/test")).await.unwrap();
        assert_eq!(output.get("vpc_id").unwrap(), &serde_json::json!("vpc-123"));
    }
}
