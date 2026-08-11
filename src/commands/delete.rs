use crate::common::deployment::{Plan, PlanRequest, PlannedUnit, Planner, WorkspaceMode};
use crate::common::project::Project;
use crate::common::*;
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct DeleteOptions {
    pub env_id: String,
    /// Override environment for specific dependencies, when Envie's record of the
    /// deployment is missing or wrong.
    pub environment_overrides: HashMap<String, String>,
    pub dry_run: bool,
    pub no_prompt: bool,
    pub verbose: bool,
}

pub struct DeleteCommand {
    working_directory: PathBuf,
    output_manager: OutputManager,
}

impl DeleteCommand {
    pub fn new(working_directory: PathBuf) -> Self {
        Self {
            working_directory,
            output_manager: OutputManager::new(),
        }
    }

    /// Destroy an environment's infrastructure and then remove its state.
    ///
    /// Unlike `destroy`, this leaves nothing behind, so the environment id can be
    /// reused from scratch. The state backend itself is never deleted: it is
    /// shared with every other environment, and on an adopted repository it
    /// belongs to the repository rather than to Envie.
    pub async fn execute(&self, options: DeleteOptions) -> Result<()> {
        let planner = Planner::new(Project::discover(&self.working_directory)?)?;
        let plan = planner.plan_teardown(&PlanRequest {
            environment: options.env_id.clone(),
            unit: None,
            environment_overrides: options.environment_overrides.clone(),
            include_dependencies: false,
            no_prompt: options.no_prompt,
            verbose: options.verbose,
        })?;

        for warning in &plan.warnings {
            self.output_manager
                .print_yellow(&format!("⚠️  {}", warning));
        }

        if plan.environment.is_stable() {
            return Err(EnvieError::ValidationError(format!(
                "'{}' is a long-lived environment declared in workspace.envie.yaml.\n\
                 `envie delete` only removes throwaway environments. To tear down '{}', run \
                 `envie destroy --env {}`, and remove it from workspace.envie.yaml if it is \
                 really going away.",
                plan.environment.name, plan.environment.name, plan.environment.name
            )));
        }

        if plan.units.is_empty() {
            self.output_manager.print_yellow("Nothing to delete.");
            return Ok(());
        }

        if options.dry_run {
            self.print_plan(&plan);
            return Ok(());
        }

        if !options.no_prompt {
            self.output_manager.print_yellow(&format!(
                "\n⚠️  This destroys everything in '{}' and deletes its state.",
                plan.environment.name
            ));
            print!("Type 'yes' to continue: ");
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            if input.trim() != "yes" {
                println!("Cancelled.");
                return Ok(());
            }
        }

        self.output_manager
            .print_blue("\nStep 1: destroying infrastructure\n");
        for unit in plan.teardown_order() {
            println!("  {}", unit.name);
            let Some(terraform) = unit.prepare(
                &plan.project_name,
                &plan.environment,
                WorkspaceMode::RequireExisting,
                options.verbose,
            )?
            else {
                println!("    ⏭️  not deployed\n");
                continue;
            };

            terraform.destroy_with_var_files(&unit.var_arguments(), &unit.var_files)?;
            terraform.workspace_select("default")?;
            terraform.workspace_delete(&plan.environment.workspace)?;
            println!("    ✅ destroyed\n");
        }

        self.output_manager.print_blue("Step 2: removing state\n");
        // Every unit's state is swept, not just the ones that were destroyed: an
        // interrupted run can leave state behind for a unit that never got as far
        // as creating anything, and leaving it would make the environment id
        // impossible to reuse cleanly.
        for unit in &self.all_units(&planner, &options)?.units {
            self.delete_state(&plan, unit, options.verbose)?;
        }
        manifest::remove(&planner.project().root, &plan.environment)?;

        self.output_manager.print_green(&format!(
            "\n✅ '{}' is gone. The state backend was left untouched.",
            plan.environment.name
        ));
        Ok(())
    }

    /// Every unit in the project, with the state path it would have in this
    /// environment. Dependency overrides are irrelevant here: where state lives
    /// depends only on the unit and the environment.
    fn all_units(&self, planner: &Planner, options: &DeleteOptions) -> Result<Plan> {
        planner.plan(&PlanRequest {
            environment: options.env_id.clone(),
            unit: None,
            environment_overrides: HashMap::new(),
            include_dependencies: false,
            no_prompt: options.no_prompt,
            verbose: options.verbose,
        })
    }

    /// Remove a unit's state object for this environment.
    ///
    /// Terraform stores a non-default workspace under `<prefix>/<workspace>/<key>`
    /// and only the default workspace at the bare key. Exactly one of those is
    /// this environment's, and the other may well be another environment's — a
    /// repository that separates environments by workspace uses the same key for
    /// all of them — so only the one in use is removed.
    fn delete_state(&self, plan: &Plan, unit: &PlannedUnit, verbose: bool) -> Result<()> {
        let Some(key) = unit.target.state_path() else {
            return Ok(());
        };

        match plan.environment.backend.backend_type.as_str() {
            "s3" => {
                let Some(bucket) = plan.environment.backend.config.get("bucket") else {
                    return Ok(());
                };
                let region = plan.environment.backend.config.get("region");
                let prefix = plan
                    .environment
                    .backend
                    .config
                    .get("workspace_key_prefix")
                    .map(String::as_str)
                    .unwrap_or("env:");

                let key = if plan.environment.workspace == "default" {
                    key.to_string()
                } else {
                    format!("{}/{}/{}", prefix, plan.environment.workspace, key)
                };

                self.delete_s3_object(bucket, &key, region.map(String::as_str), verbose)?;
            }
            "local" => {
                let path = unit.directory.join(key);
                if path.exists() {
                    std::fs::remove_file(&path)?;
                    if verbose {
                        println!("  removed {}", path.display());
                    }
                }
            }
            other => {
                self.output_manager.print_yellow(&format!(
                    "  ⚠️  cannot remove state automatically for backend '{}'; \
                     delete {} by hand if you want it gone",
                    other, key
                ));
            }
        }

        Ok(())
    }

    fn delete_s3_object(
        &self,
        bucket: &str,
        key: &str,
        region: Option<&str>,
        verbose: bool,
    ) -> Result<()> {
        let mut command = Command::new("aws");
        command.args(["s3api", "delete-object", "--bucket", bucket, "--key", key]);
        if let Some(region) = region {
            command.args(["--region", region]);
        }

        let output = command.output()?;
        if output.status.success() {
            if verbose {
                println!("  removed s3://{}/{}", bucket, key);
            }
            return Ok(());
        }

        // Deleting something that is not there is the desired end state anyway.
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("NoSuchKey") || stderr.contains("NoSuchBucket") {
            return Ok(());
        }

        Err(EnvieError::ProcessError(format!(
            "could not delete s3://{}/{}: {}",
            bucket,
            key,
            stderr.trim()
        )))
    }

    fn print_plan(&self, plan: &Plan) {
        self.output_manager
            .print_green("🗑️  Delete plan (dry run)\n");
        println!("Environment: {} (ephemeral)", plan.environment.name);
        println!("Workspace:   {}", plan.environment.workspace);
        println!();

        println!("Destroy, then delete state, in this order:");
        for (index, unit) in plan.teardown_order().iter().enumerate() {
            println!("  {}. {}", index + 1, unit.name);
            if let Some(state) = unit.target.state_path() {
                println!("     state: {}", state);
            }
        }
        println!();
        println!(
            "The {} backend itself is left alone.",
            plan.environment.backend.backend_type
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn delete_command_creation() {
        let temp_dir = TempDir::new().unwrap();
        let deleter = DeleteCommand::new(temp_dir.path().to_path_buf());
        assert_eq!(deleter.working_directory, temp_dir.path());
    }
}
