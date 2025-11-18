use crate::common::*;
use colored::*;
use std::path::PathBuf;
use std::process::Command;

pub struct DoctorCommand {
    working_directory: PathBuf,
}

#[derive(Debug)]
pub struct DoctorOptions {
    pub verbose: bool,
}

#[derive(Debug)]
enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

impl CheckStatus {
    fn icon(&self) -> &str {
        match self {
            CheckStatus::Pass => "✅",
            CheckStatus::Warn => "⚠️ ",
            CheckStatus::Fail => "❌",
        }
    }

    fn color(&self) -> Color {
        match self {
            CheckStatus::Pass => Color::Green,
            CheckStatus::Warn => Color::Yellow,
            CheckStatus::Fail => Color::Red,
        }
    }
}

impl DoctorCommand {
    pub fn new(working_directory: PathBuf) -> Self {
        Self { working_directory }
    }

    pub fn execute(&self, options: DoctorOptions) -> Result<()> {
        println!("\n{}", "🏥 Running Envie Health Checks".bold().cyan());
        println!();

        let mut total_checks = 0;
        let mut passed = 0;
        let mut warnings = 0;
        let mut failed = 0;

        // Check 1: Prerequisites
        println!("{}", "Prerequisites:".bold());
        let prereq_results = self.check_prerequisites(&options);
        for (check, status, message) in prereq_results {
            self.print_check(&check, &status, &message);
            total_checks += 1;
            match status {
                CheckStatus::Pass => passed += 1,
                CheckStatus::Warn => warnings += 1,
                CheckStatus::Fail => failed += 1,
            }
        }
        println!();

        // Check 2: Project Configuration
        println!("{}", "Project Configuration:".bold());
        let config_results = self.check_project_config(&options);
        for (check, status, message) in config_results {
            self.print_check(&check, &status, &message);
            total_checks += 1;
            match status {
                CheckStatus::Pass => passed += 1,
                CheckStatus::Warn => warnings += 1,
                CheckStatus::Fail => failed += 1,
            }
        }
        println!();

        // Check 3: Unit Discovery
        println!("{}", "Unit Discovery:".bold());
        let unit_results = self.check_units(&options);
        for (check, status, message) in unit_results {
            self.print_check(&check, &status, &message);
            total_checks += 1;
            match status {
                CheckStatus::Pass => passed += 1,
                CheckStatus::Warn => warnings += 1,
                CheckStatus::Fail => failed += 1,
            }
        }
        println!();

        // Check 4: AWS Resources (if workspace config exists)
        if self.workspace_config_exists() {
            println!("{}", "AWS Resources:".bold());
            let aws_results = self.check_aws_resources(&options);
            for (check, status, message) in aws_results {
                self.print_check(&check, &status, &message);
                total_checks += 1;
                match status {
                    CheckStatus::Pass => passed += 1,
                    CheckStatus::Warn => warnings += 1,
                    CheckStatus::Fail => failed += 1,
                }
            }
            println!();
        }

        // Summary
        self.print_summary(total_checks, passed, warnings, failed);

        // Recommendations
        if warnings > 0 || failed > 0 {
            println!();
            self.print_recommendations(failed, warnings);
        }

        if failed > 0 {
            Err(EnvieError::ValidationError(
                "Health checks failed. Please address the issues above.".to_string(),
            ))
        } else {
            Ok(())
        }
    }

    fn check_prerequisites(&self, _options: &DoctorOptions) -> Vec<(String, CheckStatus, String)> {
        let mut results = vec![];

        // Check Terraform
        match Command::new("terraform").arg("--version").output() {
            Ok(output) => {
                if output.status.success() {
                    let version = String::from_utf8_lossy(&output.stdout);
                    let version_line = version.lines().next().unwrap_or("unknown");
                    results.push((
                        "Terraform installed".to_string(),
                        CheckStatus::Pass,
                        version_line.to_string(),
                    ));
                } else {
                    results.push((
                        "Terraform installed".to_string(),
                        CheckStatus::Fail,
                        "Terraform found but returned error".to_string(),
                    ));
                }
            }
            Err(_) => {
                results.push((
                    "Terraform installed".to_string(),
                    CheckStatus::Fail,
                    "Terraform not found in PATH".to_string(),
                ));
            }
        }

        // Check Git
        match Command::new("git").arg("--version").output() {
            Ok(output) => {
                if output.status.success() {
                    let version = String::from_utf8_lossy(&output.stdout);
                    let version_line = version.lines().next().unwrap_or("unknown");
                    results.push((
                        "Git installed".to_string(),
                        CheckStatus::Pass,
                        version_line.to_string(),
                    ));
                } else {
                    results.push((
                        "Git installed".to_string(),
                        CheckStatus::Warn,
                        "Git found but returned error".to_string(),
                    ));
                }
            }
            Err(_) => {
                results.push((
                    "Git installed".to_string(),
                    CheckStatus::Warn,
                    "Git not found (optional but recommended)".to_string(),
                ));
            }
        }

        // Check AWS credentials
        match std::env::var("AWS_PROFILE")
            .or_else(|_| std::env::var("AWS_ACCESS_KEY_ID"))
        {
            Ok(_) => {
                results.push((
                    "AWS credentials configured".to_string(),
                    CheckStatus::Pass,
                    "AWS credentials found".to_string(),
                ));
            }
            Err(_) => {
                results.push((
                    "AWS credentials configured".to_string(),
                    CheckStatus::Warn,
                    "No AWS credentials found in environment".to_string(),
                ));
            }
        }

        results
    }

    fn check_project_config(&self, _options: &DoctorOptions) -> Vec<(String, CheckStatus, String)> {
        let mut results = vec![];

        // Check if workspace.envie.yaml exists
        let workspace_config_path = self.working_directory.join("workspace.envie.yaml");
        if workspace_config_path.exists() {
            results.push((
                "workspace.envie.yaml exists".to_string(),
                CheckStatus::Pass,
                workspace_config_path.display().to_string(),
            ));

            // Try to parse it
            match std::fs::read_to_string(&workspace_config_path) {
                Ok(content) => match serde_yaml::from_str::<serde_yaml::Value>(&content) {
                    Ok(_) => {
                        results.push((
                            "workspace.envie.yaml is valid YAML".to_string(),
                            CheckStatus::Pass,
                            "Configuration is parseable".to_string(),
                        ));
                    }
                    Err(e) => {
                        results.push((
                            "workspace.envie.yaml is valid YAML".to_string(),
                            CheckStatus::Fail,
                            format!("YAML parsing error: {}", e),
                        ));
                    }
                },
                Err(e) => {
                    results.push((
                        "workspace.envie.yaml is readable".to_string(),
                        CheckStatus::Fail,
                        format!("Cannot read file: {}", e),
                    ));
                }
            }
        } else {
            results.push((
                "workspace.envie.yaml exists".to_string(),
                CheckStatus::Fail,
                "Project not initialized. Run 'envie init'".to_string(),
            ));
        }

        // Check .gitignore
        let gitignore_path = self.working_directory.join(".gitignore");
        if gitignore_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&gitignore_path) {
                if content.contains("envie-backend.tf")
                    || content.contains("envie-remote-state.tf")
                {
                    results.push((
                        ".gitignore has Envie patterns".to_string(),
                        CheckStatus::Pass,
                        "Envie generated files will be ignored".to_string(),
                    ));
                } else {
                    results.push((
                        ".gitignore has Envie patterns".to_string(),
                        CheckStatus::Warn,
                        "Missing Envie file patterns".to_string(),
                    ));
                }
            }
        } else {
            results.push((
                ".gitignore exists".to_string(),
                CheckStatus::Warn,
                "No .gitignore found".to_string(),
            ));
        }

        results
    }

    fn check_units(&self, _options: &DoctorOptions) -> Vec<(String, CheckStatus, String)> {
        let mut results = vec![];

        // Try to discover units
        let mut discovery = unit_discovery::UnitDiscovery::new(self.working_directory.clone());

        // Perform discovery
        match discovery.discover_all() {
            Ok(_) => {
                let unit_count = discovery.get_all_units().len();
                if unit_count > 0 {
                    results.push((
                        format!("Found {} unit(s)", unit_count),
                        CheckStatus::Pass,
                        "Units discovered successfully".to_string(),
                    ));

                    // Check for dependency cycles
                    match discovery.get_units_in_dependency_order() {
                        Ok(_) => {
                            results.push((
                                "Dependency graph is valid".to_string(),
                                CheckStatus::Pass,
                                "No circular dependencies detected".to_string(),
                            ));
                        }
                        Err(e) => {
                            results.push((
                                "Dependency graph is valid".to_string(),
                                CheckStatus::Fail,
                                format!("Circular dependency detected: {}", e),
                            ));
                        }
                    }

                    // Check for units without descriptions
                    let all_units = discovery.get_all_units();
                    let units_without_desc_count = all_units
                        .iter()
                        .filter(|u| u.config.description.is_empty())
                        .count();

                    if units_without_desc_count > 0 {
                        results.push((
                            "All units have descriptions".to_string(),
                            CheckStatus::Warn,
                            format!(
                                "{} unit(s) missing descriptions",
                                units_without_desc_count
                            ),
                        ));
                    } else {
                        results.push((
                            "All units have descriptions".to_string(),
                            CheckStatus::Pass,
                            "Documentation complete".to_string(),
                        ));
                    }
                } else {
                    results.push((
                        "Found units".to_string(),
                        CheckStatus::Warn,
                        "No units found. Create some with 'envie init'".to_string(),
                    ));
                }
            }
            Err(e) => {
                results.push((
                    "Unit discovery".to_string(),
                    CheckStatus::Fail,
                    format!("Failed during discovery: {}", e),
                ));
            }
        }

        results
    }

    fn check_aws_resources(&self, _options: &DoctorOptions) -> Vec<(String, CheckStatus, String)> {
        let mut results = vec![];

        // Try to load workspace config
        let workspace_config_path = self.working_directory.join("workspace.envie.yaml");
        let config = match std::fs::read_to_string(&workspace_config_path) {
            Ok(content) => match serde_yaml::from_str::<service_config::WorkspaceConfig>(&content) {
                Ok(cfg) => cfg,
                Err(e) => {
                    results.push((
                        "Workspace configuration".to_string(),
                        CheckStatus::Fail,
                        format!("Failed to parse config: {}", e),
                    ));
                    return results;
                }
            },
            Err(e) => {
                results.push((
                    "Workspace configuration".to_string(),
                    CheckStatus::Fail,
                    format!("Failed to read config: {}", e),
                ));
                return results;
            }
        };

        // Check environments configuration
        if let Some(ref environments) = config.environments {
            // Check ephemeral backend
            let backend_config = &environments.ephemeral.backend.config;
            if let Some(bucket) = backend_config.get("bucket") {
                results.push((
                    format!("S3 bucket configured: {}", bucket),
                    CheckStatus::Pass,
                    "Ephemeral backend configured".to_string(),
                ));
            }

            let stable_env_count = environments.stable.len();
            if stable_env_count > 0 {
                results.push((
                    format!("{} stable environment(s) configured", stable_env_count),
                    CheckStatus::Pass,
                    "Stable environments available".to_string(),
                ));
            } else {
                results.push((
                    "Stable environments configured".to_string(),
                    CheckStatus::Warn,
                    "No stable environments defined".to_string(),
                ));
            }
        } else {
            results.push((
                "Environments configuration".to_string(),
                CheckStatus::Warn,
                "No environments configured in workspace.envie.yaml".to_string(),
            ));
        }

        results
    }

    fn workspace_config_exists(&self) -> bool {
        self.working_directory
            .join("workspace.envie.yaml")
            .exists()
    }

    fn print_check(&self, check: &str, status: &CheckStatus, message: &str) {
        let status_icon = status.icon();
        let status_color = status.color();

        println!(
            "  {} {}",
            status_icon,
            check.color(status_color)
        );
        if !message.is_empty() {
            println!("     {}", message.dimmed());
        }
    }

    fn print_summary(&self, total: usize, passed: usize, warnings: usize, failed: usize) {
        println!("{}", "Summary:".bold());
        println!(
            "  Total checks: {}",
            total.to_string().bold()
        );
        println!(
            "  {} Passed: {}",
            "✅",
            passed.to_string().green().bold()
        );
        if warnings > 0 {
            println!(
                "  {} Warnings: {}",
                "⚠️ ",
                warnings.to_string().yellow().bold()
            );
        }
        if failed > 0 {
            println!(
                "  {} Failed: {}",
                "❌",
                failed.to_string().red().bold()
            );
        }

        println!();
        if failed == 0 && warnings == 0 {
            println!("{}", "Overall: ✅ Healthy".green().bold());
        } else if failed == 0 {
            println!(
                "{}",
                format!("Overall: ⚠️  Healthy ({} warning(s))", warnings)
                    .yellow()
                    .bold()
            );
        } else {
            println!(
                "{}",
                format!("Overall: ❌ Issues Found ({} failure(s))", failed)
                    .red()
                    .bold()
            );
        }
    }

    fn print_recommendations(&self, failed: usize, warnings: usize) {
        println!("{}", "Recommendations:".bold().cyan());

        if failed > 0 {
            println!("  {} Address failed checks above before deploying", "•".cyan());
        }

        if warnings > 0 {
            println!(
                "  {} Review warnings to improve project setup",
                "•".cyan()
            );
        }

        println!("\n{}", "Common fixes:".bold());
        println!("  {} Install Terraform: https://www.terraform.io/downloads", "•".dimmed());
        println!("  {} Configure AWS credentials: aws configure", "•".dimmed());
        println!("  {} Initialize project: envie init", "•".dimmed());
        println!("  {} View units: envie list", "•".dimmed());
    }
}
