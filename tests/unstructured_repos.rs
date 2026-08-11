//! Adoption and planning against repository layouts Envie did not create.
//!
//! Each test builds a Terraform repository in a temporary directory, adopts it,
//! and then asks the planner what it would do. Terraform is never invoked, so
//! these run without credentials or network access.

use envie::commands::adopt::{AdoptCommand, AdoptOptions};
use envie::common::deployment::{PlanRequest, Planner};
use envie::common::project::Project;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn write(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn adopt(root: &Path, environment: Option<&str>) {
    adopt_environments(root, environment.map(str::to_string).into_iter().collect())
        .expect("adoption should succeed");
}

fn adopt_environments(root: &Path, environments: Vec<String>) -> envie::common::Result<()> {
    AdoptCommand::new(root.to_path_buf()).execute(AdoptOptions {
        project_name: None,
        environments,
        dry_run: false,
        force: false,
        verbose: false,
    })
}

fn plan_for(root: &Path, environment: &str) -> envie::common::deployment::Plan {
    plan_with_overrides(root, environment, HashMap::new(), None)
}

fn plan_with_overrides(
    root: &Path,
    environment: &str,
    environment_overrides: HashMap<String, String>,
    unit: Option<&str>,
) -> envie::common::deployment::Plan {
    let project = Project::discover(root).expect("adopted project should be discoverable");
    Planner::new(project)
        .expect("planner should build")
        .plan(&PlanRequest {
            environment: environment.to_string(),
            unit: unit.map(str::to_string),
            environment_overrides,
            include_dependencies: true,
            no_prompt: true,
            verbose: false,
        })
        .expect("planning should succeed")
}

fn teardown_plan(root: &Path, environment: &str) -> envie::common::deployment::Plan {
    let project = Project::discover(root).expect("adopted project should be discoverable");
    Planner::new(project)
        .expect("planner should build")
        .plan_teardown(&PlanRequest {
            environment: environment.to_string(),
            unit: None,
            environment_overrides: HashMap::new(),
            include_dependencies: false,
            no_prompt: true,
            verbose: false,
        })
        .expect("teardown planning should succeed")
}

fn state_key_of(plan: &envie::common::deployment::Plan, unit: &str) -> String {
    let unit = plan
        .units
        .iter()
        .find(|u| u.name == unit)
        .unwrap_or_else(|| panic!("unit {} not in plan: {:?}", unit, unit_names(plan)));
    unit.target
        .config
        .get("key")
        .cloned()
        .unwrap_or_else(|| unit.target.config.get("path").cloned().unwrap_or_default())
}

fn unit_names(plan: &envie::common::deployment::Plan) -> Vec<String> {
    plan.units.iter().map(|u| u.name.clone()).collect()
}

fn vars_of(
    plan: &envie::common::deployment::Plan,
    unit: &str,
) -> std::collections::BTreeMap<String, String> {
    plan.units
        .iter()
        .find(|u| u.name == unit)
        .unwrap_or_else(|| panic!("unit {} not in plan: {:?}", unit, unit_names(plan)))
        .vars
        .clone()
}

/// A single root module at the repository root, with no backend at all.
#[test]
fn adopts_a_flat_repository_with_local_state() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write(
        root,
        "main.tf",
        r#"
variable "environment" { type = string }

resource "aws_ssm_parameter" "greeting" {
  name  = "/${var.environment}/greeting"
  type  = "String"
  value = "hello"
}

output "name" { value = aws_ssm_parameter.greeting.name }
"#,
    );

    adopt(root, Some("prod"));

    assert!(root.join("workspace.envie.yaml").exists());
    assert!(root.join("envie.yaml").exists());

    let plan = plan_for(root, "pr-7");
    assert_eq!(unit_names(&plan).len(), 1);
    assert_eq!(
        plan.units[0].vars.get("environment").map(String::as_str),
        Some("pr-7"),
        "the repository's own environment variable should be wired to the environment id"
    );
}

/// A flat repository is the case most likely to have its pinned state path missed,
/// because its unit is spelled `""`, `"."` and `root` in different places. Missing
/// it would point the adopted environment at a fresh state file and rebuild
/// everything.
#[test]
fn preserves_the_existing_state_path_of_a_flat_repository() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write(
        root,
        "main.tf",
        r#"
terraform {
  backend "s3" {
    bucket = "acme-tf-state"
    key    = "prod/terraform.tfstate"
    region = "eu-west-1"
  }
}

variable "environment" { type = string }

resource "aws_ssm_parameter" "p" {
  name  = "/${var.environment}/p"
  type  = "String"
  value = "v"
}
"#,
    );

    adopt(root, Some("prod"));

    assert_eq!(
        state_key_of(&plan_for(root, "prod"), "root"),
        "prod/terraform.tfstate",
        "the flat root's existing state must still be the state Envie uses"
    );
}

/// `live/` roots plus a shared `modules/` tree: the module directory is not a
/// deployable unit even though it has .tf files.
#[test]
fn does_not_adopt_shared_modules_as_units() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    write(
        root,
        "modules/bucket/main.tf",
        r#"
variable "name" { type = string }
output "id" { value = var.name }
"#,
    );
    write(
        root,
        "live/network/main.tf",
        r#"
terraform {
  backend "s3" {
    bucket = "acme-tf-state"
    key    = "prod/network/terraform.tfstate"
    region = "eu-west-1"
  }
}

variable "env" { type = string }

module "bucket" {
  source = "../../modules/bucket"
  name   = "acme-${var.env}"
}

resource "aws_ssm_parameter" "vpc" {
  name  = "/${var.env}/vpc"
  type  = "String"
  value = "vpc-1"
}

output "vpc_id" { value = aws_ssm_parameter.vpc.value }
"#,
    );
    write(
        root,
        "live/app/main.tf",
        r#"
terraform {
  backend "s3" {
    bucket = "acme-tf-state"
    key    = "prod/app/terraform.tfstate"
    region = "eu-west-1"
  }
}

variable "env" { type = string }

data "terraform_remote_state" "network" {
  backend = "s3"
  config = {
    bucket = "acme-tf-state"
    key    = "prod/network/terraform.tfstate"
    region = "eu-west-1"
  }
}

resource "aws_ssm_parameter" "app" {
  name  = "/${var.env}/app"
  type  = "String"
  value = data.terraform_remote_state.network.outputs.vpc_id
}
"#,
    );

    adopt(root, Some("prod"));

    assert!(
        !root.join("modules/bucket/envie.yaml").exists(),
        "a directory used as a module source is not a deployable unit"
    );
    assert!(root.join("live/network/envie.yaml").exists());
    assert!(root.join("live/app/envie.yaml").exists());

    let plan = plan_for(root, "prod");
    assert_eq!(
        unit_names(&plan),
        vec!["network".to_string(), "app".to_string()],
        "the remote state read should order app after network"
    );
}

/// Adoption must keep the existing environment pointed at the state paths the
/// repository already uses, or the first deploy would rebuild everything.
#[test]
fn preserves_existing_state_paths_for_the_adopted_environment() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write(
        root,
        "infra/main.tf",
        r#"
terraform {
  backend "s3" {
    bucket = "legacy-state"
    key    = "some/legacy/path/terraform.tfstate"
    region = "us-east-1"
  }
}

resource "aws_ssm_parameter" "p" {
  name  = "/legacy/p"
  type  = "String"
  value = "v"
}
"#,
    );

    adopt(root, Some("production"));

    let adopted = plan_for(root, "production");
    assert_eq!(
        state_key_of(&adopted, "infra"),
        "some/legacy/path/terraform.tfstate",
        "the adopted environment keeps the literal key the repository already had"
    );
    assert_eq!(
        adopted.environment.workspace, "default",
        "an adopted environment stays in the default workspace so S3 keys do not move"
    );

    let ephemeral = plan_for(root, "pr-3");
    assert_eq!(
        state_key_of(&ephemeral, "infra"),
        "envie/ephemeral/pr-3/infra/terraform.tfstate",
        "new environments get their own state, derived from the unit path"
    );
}

/// Copy-pasted per-environment directories are adopted as they are: each stays
/// its own unit, and nothing is collapsed or moved.
#[test]
fn adopts_copied_environment_directories_in_place() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    for env in ["dev", "prod"] {
        write(
            root,
            &format!("envs/{}/main.tf", env),
            &format!(
                r#"
terraform {{
  backend "s3" {{
    bucket = "acme-tf-state"
    key    = "{env}/terraform.tfstate"
    region = "eu-west-1"
  }}
}}

variable "stage" {{
  type    = string
  default = "{env}"
}}

resource "aws_ssm_parameter" "p" {{
  name  = "/${{var.stage}}/p"
  type  = "String"
  value = "v"
}}
"#,
                env = env
            ),
        );
    }

    adopt(root, Some("prod"));

    let mut units: Vec<PathBuf> = ["envs/dev", "envs/prod"]
        .iter()
        .map(|p| root.join(p).join("envie.yaml"))
        .collect();
    units.retain(|p| p.exists());
    assert_eq!(units.len(), 2, "both copied directories become units");

    let plan = plan_for(root, "prod");
    let names = unit_names(&plan);
    assert!(names.contains(&"dev".to_string()) && names.contains(&"prod".to_string()));
    assert_eq!(
        state_key_of(&plan, "dev"),
        "dev/terraform.tfstate",
        "existing state is left exactly where it is"
    );
}

/// Adopting one of several environment directories must not rewrite the others.
///
/// Each directory names its own environment, so injecting the adopted name into
/// all of them would run dev's code as production against dev's state.
#[test]
fn adopting_one_environment_directory_leaves_the_others_alone() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write_environment_directories(root);

    adopt(root, Some("prod"));

    let plan = plan_for(root, "prod");
    for unit in ["dev", "prod"] {
        assert_eq!(
            vars_of(&plan, unit).get("stage"),
            None,
            "{unit} keeps the value its own directory declares"
        );
    }

    // A brand new environment is a copy, and does need to be told its name.
    let ephemeral = plan_with_overrides(root, "pr-1", HashMap::new(), Some("dev"));
    assert_eq!(
        vars_of(&ephemeral, "dev").get("stage"),
        Some(&"pr-1".to_string())
    );
}

/// A `backend "s3" {}` block configured by `-backend-config` at init time.
#[test]
fn reads_a_backend_kept_in_a_separate_settings_file() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    write(
        root,
        "main.tf",
        r#"
terraform {
  backend "s3" {}
}

variable "environment" {
  type = string
}

resource "aws_ssm_parameter" "p" {
  name  = "/${var.environment}/p"
  type  = "String"
  value = "v"
}
"#,
    );
    for environment in ["prod", "staging"] {
        write(
            root,
            &format!("config/{}.s3.tfbackend", environment),
            &format!(
                "bucket = \"acme-tf-state\"\nkey = \"{}/terraform.tfstate\"\nregion = \"eu-west-1\"\n",
                environment
            ),
        );
    }

    adopt_environments(root, vec!["prod".to_string(), "staging".to_string()])
        .expect("adoption should succeed");

    assert_eq!(
        state_key_of(&plan_for(root, "prod"), "root"),
        "prod/terraform.tfstate",
        "the bucket and key live only in the settings file"
    );
    assert_eq!(
        state_key_of(&plan_for(root, "staging"), "root"),
        "staging/terraform.tfstate",
        "every environment with a settings file is adopted, not just the first"
    );
}

/// A directory called `terraform` describes the layout, not the unit.
#[test]
fn names_a_unit_after_its_service_rather_than_its_terraform_directory() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    write(
        root,
        "services/api/terraform/main.tf",
        r#"
terraform {
  backend "s3" {
    bucket = "acme-tf-state"
    key    = "api/terraform.tfstate"
    region = "eu-west-1"
  }
}

resource "aws_ssm_parameter" "p" {
  name  = "/api/p"
  type  = "String"
  value = "v"
}
"#,
    );

    adopt(root, Some("prod"));

    assert_eq!(unit_names(&plan_for(root, "prod")), vec!["api".to_string()]);
}

fn write_environment_directories(root: &Path) {
    for environment in ["dev", "prod"] {
        write(
            root,
            &format!("envs/{}/main.tf", environment),
            &format!(
                r#"
terraform {{
  backend "s3" {{
    bucket = "acme-tf-state"
    key    = "{environment}/terraform.tfstate"
    region = "eu-west-1"
  }}
}}

variable "stage" {{
  type    = string
  default = "{environment}"
}}

resource "aws_ssm_parameter" "p" {{
  name  = "/${{var.stage}}/p"
  type  = "String"
  value = "v"
}}
"#,
                environment = environment
            ),
        );
    }
}

/// `-E unit:environment` points a dependency at another environment's state and
/// drops that unit from the deployment.
#[test]
fn reads_a_dependency_from_another_environment() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    write(
        root,
        "network/main.tf",
        r#"
terraform {
  backend "s3" {
    bucket = "acme-tf-state"
    key    = "prod/network/terraform.tfstate"
    region = "eu-west-1"
  }
}
resource "aws_ssm_parameter" "vpc" {
  name  = "/vpc"
  type  = "String"
  value = "vpc-1"
}
output "vpc_id" { value = aws_ssm_parameter.vpc.value }
"#,
    );
    write(
        root,
        "app/main.tf",
        r#"
terraform {
  backend "s3" {
    bucket = "acme-tf-state"
    key    = "prod/app/terraform.tfstate"
    region = "eu-west-1"
  }
}
data "terraform_remote_state" "network" {
  backend = "s3"
  config = {
    bucket = "acme-tf-state"
    key    = "prod/network/terraform.tfstate"
    region = "eu-west-1"
  }
}
resource "aws_ssm_parameter" "app" {
  name  = "/app"
  type  = "String"
  value = data.terraform_remote_state.network.outputs.vpc_id
}
"#,
    );

    adopt(root, Some("prod"));

    let mut overrides = HashMap::new();
    overrides.insert("network".to_string(), "stable.prod".to_string());
    let plan = plan_with_overrides(root, "pr-9", overrides, Some("app"));

    assert_eq!(
        unit_names(&plan),
        vec!["app".to_string()],
        "a redirected dependency is not deployed alongside the unit that reads it"
    );

    let dependency = &plan.units[0].dependencies[0];
    assert!(dependency.overridden);
    assert_eq!(
        dependency.state.config.get("key").map(String::as_str),
        Some("prod/network/terraform.tfstate"),
        "the override should read the stable environment's state"
    );
}

/// Two roots with a local backend, so state and Envie's own record stay on disk.
fn local_backend_repository(root: &Path) {
    write(
        root,
        "network/main.tf",
        r#"
resource "aws_ssm_parameter" "vpc" {
  name  = "/vpc"
  type  = "String"
  value = "vpc-1"
}
output "vpc_id" { value = aws_ssm_parameter.vpc.value }
"#,
    );
    write(
        root,
        "app/main.tf",
        r#"
data "terraform_remote_state" "network" {
  backend = "local"
  config = {
    path = "../network/terraform.tfstate"
  }
}
resource "aws_ssm_parameter" "app" {
  name  = "/app"
  type  = "String"
  value = data.terraform_remote_state.network.outputs.vpc_id
}
"#,
    );
}

/// Tearing down must reuse the wiring the deploy recorded. Without it, a unit
/// that read another environment's state would be destroyed against state that
/// was never written, and Terraform cannot evaluate what to remove.
#[test]
fn teardown_replays_the_recorded_deployment() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    local_backend_repository(root);
    adopt(root, Some("prod"));

    // Stand in for `envie deploy --env pr-4 --unit app -E network:stable.prod`.
    let deployed = plan_with_overrides(
        root,
        "pr-4",
        HashMap::from([("network".to_string(), "stable.prod".to_string())]),
        Some("app"),
    );
    envie::common::manifest::save(
        root,
        &deployed.environment,
        envie::common::manifest::EnvironmentManifest::from_plan(&deployed),
    )
    .unwrap();

    // Teardown is asked for nothing but the environment name.
    let teardown = teardown_plan(root, "pr-4");

    assert_eq!(
        unit_names(&teardown),
        vec!["app".to_string()],
        "only the units that were deployed are torn down"
    );
    let dependency = &teardown.units[0].dependencies[0];
    assert_eq!(
        dependency.environment_reference, "stable.prod",
        "the dependency is read from where the deploy read it"
    );
    assert!(
        teardown.warnings.is_empty(),
        "a recorded environment needs no warning, got {:?}",
        teardown.warnings
    );
}

/// An environment deployed before Envie kept records, or by another tool, still
/// tears down — with a warning rather than a wrong assumption.
#[test]
fn teardown_warns_when_there_is_no_record() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    local_backend_repository(root);
    adopt(root, Some("prod"));

    let teardown = teardown_plan(root, "pr-5");

    assert_eq!(unit_names(&teardown).len(), 2, "falls back to every unit");
    assert!(
        teardown
            .warnings
            .iter()
            .any(|warning| warning.contains("no record of how")),
        "the user should be told the wiring is a guess, got {:?}",
        teardown.warnings
    );
}

/// Deploying one unit must not make Envie forget the rest of the environment.
#[test]
fn recording_a_single_unit_keeps_the_rest_of_the_environment() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    local_backend_repository(root);
    adopt(root, Some("prod"));

    let everything = plan_for(root, "pr-6");
    envie::common::manifest::save(
        root,
        &everything.environment,
        envie::common::manifest::EnvironmentManifest::from_plan(&everything),
    )
    .unwrap();

    let just_app = plan_with_overrides(root, "pr-6", HashMap::new(), Some("app"));
    envie::common::manifest::save(
        root,
        &just_app.environment,
        envie::common::manifest::EnvironmentManifest::from_plan(&just_app),
    )
    .unwrap();

    let teardown = teardown_plan(root, "pr-6");
    let mut names = unit_names(&teardown);
    names.sort();
    assert_eq!(names, vec!["app".to_string(), "network".to_string()]);
}

/// A repository that separates environments with Terraform workspaces, naming
/// resources from `terraform.workspace`. Adopting it into the `default` workspace
/// would rename every resource, so the workspace name has to be kept.
#[test]
fn adopts_a_workspace_separated_repository() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write(
        root,
        "main.tf",
        r#"
terraform {
  backend "s3" {
    bucket               = "acme-tf-state"
    key                  = "terraform.tfstate"
    region               = "eu-west-1"
    workspace_key_prefix = "env"
  }
}

locals {
  env = terraform.workspace
}

resource "aws_sqs_queue" "jobs" {
  name = "jobs-${local.env}"
}
"#,
    );

    adopt(root, Some("prod"));

    let adopted = plan_for(root, "prod");
    assert_eq!(
        adopted.environment.workspace, "prod",
        "the adopted environment keeps the workspace its state is in"
    );
    assert_eq!(
        state_key_of(&adopted, "root"),
        "terraform.tfstate",
        "the key is unchanged; the backend's workspace prefix separates environments"
    );
    assert_eq!(
        adopted
            .environment
            .backend
            .config
            .get("workspace_key_prefix")
            .map(String::as_str),
        Some("env"),
        "dropping the prefix would move the existing state"
    );

    // A new environment is just another workspace, exactly as the repository
    // would have made one by hand.
    let ephemeral = plan_for(root, "pr-3");
    assert_eq!(ephemeral.environment.workspace, "pr-3");
    assert_eq!(state_key_of(&ephemeral, "root"), "terraform.tfstate");
}

/// Adoption writes only its own files; the repository's Terraform is untouched.
#[test]
fn leaves_existing_terraform_files_alone() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    let original = r#"
terraform {
  backend "s3" {
    bucket = "acme-tf-state"
    key    = "prod/only/terraform.tfstate"
    region = "eu-west-1"
  }
}
resource "aws_ssm_parameter" "p" {
  name  = "/p"
  type  = "String"
  value = "v"
}
"#;
    write(root, "only/main.tf", original);

    adopt(root, Some("prod"));

    assert_eq!(
        fs::read_to_string(root.join("only/main.tf")).unwrap(),
        original
    );
}

/// A second adoption without --force refuses rather than overwriting.
#[test]
fn refuses_to_re_adopt_without_force() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write(
        root,
        "main.tf",
        r#"
resource "aws_ssm_parameter" "p" {
  name  = "/p"
  type  = "String"
  value = "v"
}
"#,
    );

    adopt(root, Some("prod"));

    let second = adopt_environments(root, vec!["prod".to_string()]);
    assert!(second.is_err(), "re-adoption should require --force");
}

/// A repository with no Terraform in it is a clear error, not an empty project.
#[test]
fn reports_when_there_is_no_terraform_to_adopt() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "README.md", "no terraform here");

    let result = AdoptCommand::new(dir.path().to_path_buf()).execute(AdoptOptions {
        project_name: None,
        environments: Vec::new(),
        dry_run: true,
        force: false,
        verbose: false,
    });
    assert!(result.is_err());
}

/// A repository that already has more than one long-lived environment gets them
/// all declared, rather than one plus a hand edit.
#[test]
fn declares_every_environment_it_is_given() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write(
        root,
        "main.tf",
        r#"
terraform {
  backend "s3" {
    bucket               = "acme-tf-state"
    key                  = "terraform.tfstate"
    region               = "eu-west-1"
    workspace_key_prefix = "env"
  }
}
resource "aws_sqs_queue" "jobs" {
  name = "jobs-${terraform.workspace}"
}
"#,
    );

    adopt_environments(
        root,
        vec!["prod".to_string(), "staging".to_string(), "dev".to_string()],
    )
    .unwrap();

    for (environment, workspace) in [("prod", "prod"), ("staging", "staging"), ("dev", "dev")] {
        let plan = plan_for(root, environment);
        assert!(
            plan.environment.is_stable(),
            "{} should be a declared long-lived environment",
            environment
        );
        assert_eq!(plan.environment.workspace, workspace);
    }
}
