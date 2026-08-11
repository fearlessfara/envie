//! Scaffolds a new Envie project.
//!
//! This is the path for a repository with no Terraform in it yet. A repository
//! that already has Terraform should use `envie adopt`, which reads what is there
//! rather than writing something new.
//!
//! What is written is deliberately small and deliberately runnable: two units,
//! one reading the other, on a local backend and using only Terraform's built-in
//! `terraform_data`. `envie deploy --env pr-1` works immediately, with no cloud
//! account and no credentials, so the shape of a project can be seen before any
//! of it is real.

use crate::common::error::EnvieError;
use crate::common::Result;
use std::io::{self, Write};

#[derive(Debug, Clone)]
pub struct InitOptions {
    pub name: Option<String>,
    pub description: Option<String>,
    pub no_prompt: bool,
    pub verbose: bool,
}

pub struct InitCommand {
    working_directory: std::path::PathBuf,
}

/// A unit written by `envie init`, and the Terraform that makes it work.
struct ScaffoldUnit {
    name: &'static str,
    description: &'static str,
    /// Units this one reads, by name.
    dependencies: &'static [&'static str],
    main_tf: &'static str,
}

const UNITS: &[ScaffoldUnit] = &[
    ScaffoldUnit {
        name: "db",
        description: "Stands in for a database. Produces a name other units read.",
        dependencies: &[],
        main_tf: r#"# Every environment gets its own copy of this unit, so anything named here
# has to vary by environment. local.envie_environment_id is written by Envie
# into envie.generated.tf on each deploy, and is the environment's id.
resource "terraform_data" "table" {
  input = "${local.envie_project_name}-${local.envie_environment_id}-items"
}

output "table_name" {
  description = "Name of this environment's table"
  value       = terraform_data.table.output
}
"#,
    },
    ScaffoldUnit {
        name: "api",
        description: "Stands in for an API. Reads the db unit's output.",
        dependencies: &["db"],
        main_tf: r#"# The dependency declared in envie.yaml becomes a terraform_remote_state data
# source in envie_override.tf, pointing at whichever environment is being
# deployed. Nothing here has to name an environment.
resource "terraform_data" "endpoint" {
  input = "https://${local.envie_environment_id}.example.internal"
}

output "endpoint" {
  description = "Where this environment's API would answer"
  value       = terraform_data.endpoint.output
}

output "reads_table" {
  description = "The table this environment's API was wired to"
  value       = data.terraform_remote_state.db.outputs.table_name
}
"#,
    },
];

impl InitCommand {
    pub fn new(working_directory: std::path::PathBuf) -> Self {
        Self { working_directory }
    }

    pub async fn execute(&self, options: InitOptions) -> Result<()> {
        if options.verbose {
            println!("🚀 Initializing Envie project...");
        }

        // Overwriting an existing configuration is how someone loses the
        // state_keys that point an adopted repository at infrastructure it
        // already has, so this never happens without being asked for. Without a
        // terminal to ask, it stops.
        if self.is_already_initialized()? {
            if options.no_prompt {
                return Err(EnvieError::ValidationError(format!(
                    "{} already exists, and overwriting it would discard the \
                     environments configured there.\n\
                     Remove it first if you meant to start over.",
                    self.working_directory
                        .join("workspace.envie.yaml")
                        .display()
                )));
            }

            print!("This project is already initialized. Overwrite its configuration? [y/N]: ");
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            if !input.trim().to_lowercase().starts_with('y') {
                println!("Initialization cancelled.");
                return Ok(());
            }
        }

        let (name, description) = self.project_info(&options)?;

        std::fs::write(
            self.working_directory.join("workspace.envie.yaml"),
            workspace_config(&name, &description),
        )?;

        for unit in UNITS {
            let dir = self.working_directory.join("units").join(unit.name);
            std::fs::create_dir_all(&dir)?;
            std::fs::write(dir.join("envie.yaml"), unit_config(unit))?;
            std::fs::write(dir.join("main.tf"), unit.main_tf)?;
        }

        self.update_gitignore()?;
        std::fs::write(
            self.working_directory.join("README.md"),
            readme(&name, &description),
        )?;

        println!(
            "\n✅ Created an Envie project in {}",
            self.working_directory.display()
        );
        println!();
        println!("  workspace.envie.yaml   project, environments and backend");
        println!("  units/db/              a unit that produces an output");
        println!("  units/api/             a unit that reads it");
        println!();
        println!("The two units use Terraform's built-in terraform_data, so they cost");
        println!("nothing to deploy. With AWS credentials in your shell:");
        println!();
        println!("  envie deploy --env pr-1 --dry-run   # what would happen");
        println!("  envie deploy --env pr-1             # build a whole environment");
        println!("  envie output --env pr-1             # see what it produced");
        println!("  envie delete --env pr-1             # remove it again");
        println!();
        println!("Check the bucket name in workspace.envie.yaml first — S3 bucket names are");
        println!("global. Then replace the units with your own Terraform.");

        Ok(())
    }

    fn is_already_initialized(&self) -> Result<bool> {
        Ok(self.working_directory.join("workspace.envie.yaml").exists())
    }

    fn project_info(&self, options: &InitOptions) -> Result<(String, String)> {
        let name = self.answer(
            options.name.as_deref(),
            options.no_prompt,
            "Project name",
            "my-envie-project",
        )?;
        let description = self.answer(
            options.description.as_deref(),
            options.no_prompt,
            "Project description",
            "An Envie-managed Terraform project",
        )?;
        Ok((name, description))
    }

    fn answer(
        &self,
        given: Option<&str>,
        no_prompt: bool,
        prompt: &str,
        default: &str,
    ) -> Result<String> {
        if let Some(given) = given {
            return Ok(given.to_string());
        }
        if no_prompt {
            return Ok(default.to_string());
        }

        print!("{} [{}]: ", prompt, default);
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let answer = input.trim();
        Ok(if answer.is_empty() {
            default.to_string()
        } else {
            answer.to_string()
        })
    }

    fn update_gitignore(&self) -> Result<()> {
        let path = self.working_directory.join(".gitignore");
        let existing = std::fs::read_to_string(&path).unwrap_or_default();

        let wanted = [
            ".terraform/",
            ".terraform.lock.hcl",
            "*.tfstate",
            "*.tfstate.*",
            "envie_override.tf",
            "envie.generated.tf",
            ".envie/",
        ];
        let missing: Vec<&str> = wanted
            .iter()
            .filter(|entry| !existing.lines().any(|line| line.trim() == **entry))
            .copied()
            .collect();
        if missing.is_empty() {
            return Ok(());
        }

        let mut out = existing;
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str("# Terraform, and the files Envie generates on each deploy\n");
        for entry in missing {
            out.push_str(entry);
            out.push('\n');
        }
        std::fs::write(path, out)?;
        Ok(())
    }
}

fn workspace_config(name: &str, description: &str) -> String {
    format!(
        r#"version: "1.0"

project:
  name: {name}
  description: {description}

environments:
  # One short-lived environment per feature, pull request or experiment.
  # Created by deploying to any name not listed under stable.
  ephemeral:
    naming_pattern: "{{project}}-{{id}}"
    key_pattern: "envie/ephemeral/{{id}}/{{unit_path}}/terraform.tfstate"
    backend: &backend
      type: s3
      config:
        # S3 bucket names are global, so this may need a suffix of your own.
        # Envie offers to create the bucket and the lock table on first deploy.
        bucket: {name}-tfstate
        dynamodb_table: {name}-tflocks
        encrypt: "true"
        region: eu-west-1

  stable:
    prod:
      description: The long-lived environment
      workspace: default
      backend: *backend
      key_pattern: "envie/prod/{{unit_path}}/terraform.tfstate"
"#
    )
}

fn unit_config(unit: &ScaffoldUnit) -> String {
    let mut out = format!(
        "name: {}\ndescription: {}\nunit_type: service\nstate_management: dedicated\n\n",
        unit.name, unit.description
    );

    if unit.dependencies.is_empty() {
        out.push_str("# Nothing to read from other units.\ndependencies: []\n");
    } else {
        out.push_str(
            "# Envie turns each of these into a terraform_remote_state data source\n\
             # pointing at the environment being deployed. Override one with\n\
             # -E <unit>:<environment> to read from somewhere else.\n\
             dependencies:\n",
        );
        for dependency in unit.dependencies {
            out.push_str(&format!("  - name: {}\n", dependency));
        }
    }

    out
}

fn readme(name: &str, description: &str) -> String {
    format!(
        r#"# {name}

{description}

Managed with [Envie](https://github.com/fearlessfara/envie): one Terraform
codebase, as many environments as you want.

## Layout

```text
workspace.envie.yaml   project, environments and backend
units/db/              a unit that produces an output
units/api/             a unit that reads it
```

A unit is a Terraform root module with its own state. `units/api` declares a
dependency on `db` in its `envie.yaml`, and Envie works out the order.

## Using it

```bash
envie deploy --env pr-1     # build a whole environment from scratch
envie output --env pr-1     # what it produced
envie delete --env pr-1     # remove it, resources and state
envie deploy --env prod     # the long-lived environment
```

Every environment gets its own state and its own resource names. The units get
those names from `local.envie_environment_id`, which Envie writes into
`envie.generated.tf` on each deploy — that and `envie_override.tf` are
generated, gitignored, and safe to delete.

To build one environment against another's dependencies:

```bash
envie deploy --env pr-1 --unit api -E db:stable.prod
```

## Making it real

The units only use Terraform's built-in `terraform_data`, so deploying them
costs nothing while you get a feel for the workflow. State goes to S3; Envie
offers to create the bucket and the DynamoDB lock table the first time you
deploy. Check the bucket name in `workspace.envie.yaml` before you do, since S3
bucket names are global.

Then replace the `terraform_data` resources in `units/*/main.tf` with your own
Terraform, and add units as you need them — a unit is any directory with an
`envie.yaml` next to its `.tf` files.
"#
    )
}
