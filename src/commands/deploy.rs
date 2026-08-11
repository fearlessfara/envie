use crate::common::deployment::{Plan, PlanRequest, Planner, WorkspaceMode};
use crate::common::environment::ResolvedEnvironment;
use crate::common::project::Project;
use crate::common::*;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DeployOptions {
    pub unit_name: Option<String>,
    pub env_id: String,
    pub environment_overrides: HashMap<String, String>,
    pub dry_run: bool,
    pub no_prompt: bool,
    pub verbose: bool,
}

pub struct DeployCommand {
    working_directory: PathBuf,
    output_manager: OutputManager,
}

impl DeployCommand {
    pub fn new(working_directory: PathBuf) -> Self {
        Self {
            working_directory,
            output_manager: OutputManager::new(),
        }
    }

    pub async fn execute(&self, options: DeployOptions) -> Result<()> {
        let planner = Planner::new(Project::discover(&self.working_directory)?)?;
        let plan = planner.plan(&PlanRequest {
            environment: options.env_id.clone(),
            unit: options.unit_name.clone(),
            environment_overrides: options.environment_overrides.clone(),
            // Deploying a single unit also deploys what it reads from, so it has
            // something to read.
            include_dependencies: true,
            no_prompt: options.no_prompt,
            verbose: options.verbose,
        })?;

        for warning in &plan.warnings {
            self.output_manager
                .print_yellow(&format!("⚠️  {}", warning));
        }

        if plan.units.is_empty() {
            self.output_manager
                .print_yellow("Nothing to deploy for this selection.");
            return Ok(());
        }

        if options.dry_run {
            self.print_plan(&plan);
            return Ok(());
        }

        self.ensure_backend_exists(&plan.environment, options.no_prompt, options.verbose)
            .await?;

        self.output_manager.print_green(&format!(
            "\n🚀 Deploying {} unit(s) to {}\n",
            plan.units.len(),
            plan.environment.name
        ));

        for (index, unit) in plan.units.iter().enumerate() {
            self.output_manager.print_blue(&format!(
                "[{}/{}] {}",
                index + 1,
                plan.units.len(),
                unit.name
            ));
            println!("  📍 {}", unit.path.display());
            if let Some(state) = unit.target.state_path() {
                println!("  💾 {}", state);
            }

            let terraform = unit
                .prepare(
                    &plan.project_name,
                    &plan.environment,
                    WorkspaceMode::CreateIfMissing,
                    options.verbose,
                )?
                .expect("CreateIfMissing always yields a workspace");

            println!("  ⚡ terraform apply");
            terraform.apply_with_var_files(&unit.var_arguments(), &unit.var_files)?;
            println!("  ✅ done\n");
        }

        self.record_deployment(&planner.project().root, &plan);

        self.output_manager
            .print_green(&format!("✅ {} is deployed.", plan.environment.name));
        Ok(())
    }

    /// Remember how this environment was wired, so `envie destroy` and
    /// `envie delete` can reproduce it without being told again.
    ///
    /// Failing to record it does not fail the deploy: the infrastructure is up,
    /// and teardown can still be pointed by hand with `-E`.
    fn record_deployment(&self, root: &std::path::Path, plan: &Plan) {
        let manifest = manifest::EnvironmentManifest::from_plan(plan);
        if let Err(error) = manifest::save(root, &plan.environment, manifest) {
            self.output_manager.print_yellow(&format!(
                "⚠️  Deployed, but could not record how: {}\n   \
                 Tearing this environment down may need the same -E flags used here.",
                error
            ));
        }
    }

    fn print_plan(&self, plan: &Plan) {
        self.output_manager
            .print_green("📋 Deployment plan (dry run)\n");

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
        println!("Backend:     {}", plan.environment.backend.backend_type);
        println!();

        for (index, unit) in plan.units.iter().enumerate() {
            println!("{}. {}", index + 1, unit.name);
            println!("   path:  {}", unit.path.display());
            if let Some(state) = unit.target.state_path() {
                println!("   state: {}", state);
            }
            if !unit.vars.is_empty() {
                let vars: Vec<String> = unit
                    .vars
                    .iter()
                    .map(|(key, value)| format!("{}={}", key, value))
                    .collect();
                println!("   vars:  {}", vars.join(" "));
            }
            for file in &unit.var_files {
                if unit.directory.join(file).exists() {
                    println!("   vars:  -var-file={}", file);
                }
            }
            for dependency in &unit.dependencies {
                let suffix = if dependency.overridden {
                    " (overridden)"
                } else {
                    ""
                };
                println!(
                    "   reads: {} from {}{}",
                    dependency.unit_name, dependency.environment_reference, suffix
                );
                if let Some(state) = dependency.state.state_path() {
                    println!("          {}", state);
                }
            }
            println!();
        }

        println!("{} unit(s) would be deployed.", plan.units.len());
    }

    /// Create the state bucket and lock table if the backend needs them.
    async fn ensure_backend_exists(
        &self,
        environment: &ResolvedEnvironment,
        no_prompt: bool,
        verbose: bool,
    ) -> Result<()> {
        if environment.backend.backend_type != "s3" {
            if verbose {
                println!(
                    "⚠️  Skipping backend check for backend type '{}'",
                    environment.backend.backend_type
                );
            }
            return Ok(());
        }

        let bucket = environment.backend.config.get("bucket").ok_or_else(|| {
            EnvieError::ValidationError(
                "the s3 backend needs a 'bucket' in workspace.envie.yaml".to_string(),
            )
        })?;
        let region = environment.backend.config.get("region").ok_or_else(|| {
            EnvieError::ValidationError(
                "the s3 backend needs a 'region' in workspace.envie.yaml".to_string(),
            )
        })?;
        // Optional: recent Terraform can lock through S3 itself.
        let lock_table = environment
            .backend
            .config
            .get("dynamodb_table")
            .cloned()
            .unwrap_or_default();

        let bootstrap = BackendBootstrap::new(bucket.clone(), lock_table, region.clone());
        if bootstrap.check_exists()?.is_ready() {
            if verbose {
                println!("✅ Backend infrastructure already exists");
            }
            return Ok(());
        }

        bootstrap.create(no_prompt)
    }
}
