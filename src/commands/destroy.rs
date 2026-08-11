use crate::common::deployment::{Plan, PlanRequest, Planner, WorkspaceMode};
use crate::common::project::Project;
use crate::common::*;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DestroyOptions {
    pub unit_name: Option<String>,
    pub env_id: Option<String>,
    pub environment_overrides: HashMap<String, String>,
    pub dry_run: bool,
    pub no_prompt: bool,
    pub verbose: bool,
}

pub struct DestroyCommand {
    working_directory: PathBuf,
    output_manager: OutputManager,
}

impl DestroyCommand {
    pub fn new(working_directory: PathBuf) -> Self {
        Self {
            working_directory,
            output_manager: OutputManager::new(),
        }
    }

    pub async fn execute(&self, options: DestroyOptions) -> Result<()> {
        let env_id = options.env_id.as_ref().ok_or_else(|| {
            EnvieError::ValidationError(
                "--env is required, so it is clear what is being destroyed".to_string(),
            )
        })?;

        let planner = Planner::new(Project::discover(&self.working_directory)?)?;
        let plan = planner.plan_teardown(&PlanRequest {
            environment: env_id.clone(),
            unit: options.unit_name.clone(),
            environment_overrides: options.environment_overrides.clone(),
            // Destroying a unit must not destroy what it reads from: other units
            // in other environments may still depend on it.
            include_dependencies: false,
            no_prompt: options.no_prompt,
            verbose: options.verbose,
        })?;

        for warning in &plan.warnings {
            self.output_manager
                .print_yellow(&format!("⚠️  {}", warning));
        }

        if plan.units.is_empty() {
            self.output_manager
                .print_yellow("Nothing to destroy for this selection.");
            return Ok(());
        }

        if options.dry_run {
            self.print_plan(&plan);
            return Ok(());
        }

        // Destroying a long-lived environment is not something to do by accident.
        if plan.environment.is_stable() && !options.no_prompt {
            self.confirm_stable_destroy(&plan)?;
        }

        self.output_manager.print_green(&format!(
            "\n🗑️  Destroying {} unit(s) in {}\n",
            plan.units.len(),
            plan.environment.name
        ));

        for unit in plan.teardown_order() {
            self.output_manager.print_blue(&unit.name);

            let Some(terraform) = unit.prepare(
                &plan.project_name,
                &plan.environment,
                // Nothing was ever deployed if the workspace is absent.
                WorkspaceMode::RequireExisting,
                options.verbose,
            )?
            else {
                println!("  ⏭️  not deployed in this environment\n");
                continue;
            };

            println!("  💥 terraform destroy");
            terraform.destroy_with_var_files(&unit.var_arguments(), &unit.var_files)?;

            // The workspace is only removed for throwaway environments; a stable
            // environment keeps its workspace so its history stays intact.
            if !plan.environment.is_stable() && plan.environment.workspace != "default" {
                terraform.workspace_select("default")?;
                terraform.workspace_delete(&plan.environment.workspace)?;
            }

            println!("  ✅ destroyed\n");
        }

        self.forget_deployment(&planner.project().root, &plan);

        self.output_manager
            .print_green(&format!("✅ {} is torn down.", plan.environment.name));
        Ok(())
    }

    /// Take what was just destroyed out of the deployment record, so that
    /// `envie list` stops reporting it as deployed.
    ///
    /// As when recording a deploy, the record is bookkeeping: failing to update
    /// it does not undo a teardown that worked.
    fn forget_deployment(&self, root: &std::path::Path, plan: &Plan) {
        let destroyed: Vec<String> = plan.units.iter().map(|unit| unit.name.clone()).collect();
        if let Err(error) = manifest::forget(root, &plan.environment, &destroyed) {
            self.output_manager.print_yellow(&format!(
                "⚠️  Torn down, but the record of it could not be updated: {}",
                error
            ));
        }
    }

    fn confirm_stable_destroy(&self, plan: &Plan) -> Result<()> {
        use std::io::{self, Write};

        self.output_manager.print_yellow(&format!(
            "\n⚠️  '{}' is a long-lived environment declared in workspace.envie.yaml.",
            plan.environment.name
        ));
        println!("   This destroys real infrastructure in:");
        for unit in &plan.units {
            println!(
                "     {} → {}",
                unit.name,
                unit.target.state_path().unwrap_or("<unknown state>")
            );
        }
        print!("\n   Type the environment name to confirm: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if input.trim() != plan.environment.name {
            return Err(EnvieError::ValidationError("destroy cancelled".to_string()));
        }
        Ok(())
    }

    fn print_plan(&self, plan: &Plan) {
        self.output_manager
            .print_green("🗑️  Destroy plan (dry run)\n");

        println!(
            "Environment: {} ({})",
            plan.environment.name,
            if plan.environment.is_stable() {
                "stable"
            } else {
                "ephemeral"
            }
        );
        println!("Workspace:   {}", plan.environment.workspace);
        println!();

        println!("Destroy order (dependents first):");
        for (index, unit) in plan.teardown_order().iter().enumerate() {
            println!("  {}. {}", index + 1, unit.name);
            println!("     path:  {}", unit.path.display());
            if let Some(state) = unit.target.state_path() {
                println!("     state: {}", state);
            }
        }
        println!();

        if plan.environment.is_stable() {
            self.output_manager.print_yellow(
                "This is a long-lived environment; running for real will ask for confirmation.",
            );
        } else {
            println!("The Terraform workspace will be removed afterwards.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn destroy_requires_an_environment() {
        let temp_dir = TempDir::new().unwrap();
        let destroyer = DestroyCommand::new(temp_dir.path().to_path_buf());

        let error = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(destroyer.execute(DestroyOptions {
                unit_name: None,
                env_id: None,
                environment_overrides: HashMap::new(),
                dry_run: true,
                no_prompt: true,
                verbose: false,
            }))
            .unwrap_err();

        assert!(error.to_string().contains("--env is required"), "{error}");
    }
}
