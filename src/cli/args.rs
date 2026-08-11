use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "envie")]
#[command(
    about = "A tool for managing multiple ephemeral environments in Terraform with layered dependencies and resource sharing"
)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Set up Envie on an existing Terraform repository, keeping its current state
    Adopt {
        /// Project name (defaults to the repository directory name)
        #[arg(long)]
        name: Option<String>,

        /// Long-lived environment to declare; repeat for more. The first adopts the
        /// repository's existing state.
        #[arg(long, value_name = "NAME", action = clap::ArgAction::Append)]
        environment: Vec<String>,

        /// Show what would be written without writing it
        #[arg(short = 'D', long)]
        dry_run: bool,

        /// Overwrite Envie configuration that already exists
        #[arg(long)]
        force: bool,

        /// Print detailed output during execution
        #[arg(long)]
        verbose: bool,
    },
    /// Start a new Envie project, for a repository with no Terraform yet
    Init {
        /// Project name (will prompt if not provided)
        #[arg(long)]
        name: Option<String>,

        /// Project description (will prompt if not provided)
        #[arg(long)]
        description: Option<String>,

        /// Don't prompt for inputs and use default values
        #[arg(long)]
        no_prompt: bool,

        /// Print detailed output during execution
        #[arg(long)]
        verbose: bool,
    },
    /// Deploy a unit with dependency management and Terraform orchestration
    Deploy {
        /// The name of the unit to be deployed (optional - will auto-discover from current directory)
        #[arg(short = 'U', long)]
        unit: Option<String>,

        /// The ID of the environment to deploy (e.g., MR number, feature branch, etc.)
        #[arg(long)]
        env: String,

        /// Override environment for specific dependencies (format: unit:environment)
        #[arg(short = 'E', long, action = clap::ArgAction::Append)]
        environment: Vec<String>,

        /// Simulate the deployment process without making changes
        #[arg(short = 'D', long)]
        dry_run: bool,

        /// Don't prompt for inputs and use default values
        #[arg(long)]
        no_prompt: bool,

        /// Print detailed output during execution
        #[arg(long)]
        verbose: bool,
    },
    /// Destroy the infrastructure of an environment
    Destroy {
        /// The name of the unit to destroy (optional - will auto-discover from current directory)
        #[arg(short = 'U', long)]
        unit: Option<String>,

        /// The ID of the environment to destroy
        #[arg(long)]
        env: Option<String>,

        /// Override environment for specific dependencies (format: unit:environment)
        #[arg(short = 'E', long, action = clap::ArgAction::Append)]
        environment: Vec<String>,

        /// Simulate the destruction process without making changes
        #[arg(short = 'D', long)]
        dry_run: bool,

        /// Don't prompt for confirmation
        #[arg(long)]
        no_prompt: bool,

        /// Print detailed output during execution
        #[arg(long)]
        verbose: bool,
    },
    /// Destroy an ephemeral environment and remove its state, leaving the backend alone
    Delete {
        /// The name of the unit to delete (optional - deletes all units if not specified)
        #[arg(short = 'U', long)]
        unit: Option<String>,

        /// The ID of the environment to delete
        #[arg(long)]
        env: String,

        /// Override environment for specific dependencies (format: unit:environment)
        #[arg(short = 'E', long, action = clap::ArgAction::Append)]
        environment: Vec<String>,

        /// Simulate the deletion process without making changes
        #[arg(short = 'D', long)]
        dry_run: bool,

        /// Don't prompt for confirmation
        #[arg(long)]
        no_prompt: bool,

        /// Print detailed output during execution
        #[arg(long)]
        verbose: bool,
    },
    /// Generate environment variables from Terraform outputs
    Generate {
        /// The environment to read outputs from
        #[arg(long)]
        env: Option<String>,

        /// Path to the environment file template
        #[arg(long, default_value = ".env.example")]
        env_file: PathBuf,

        /// Read outputs from this file instead of from the deployed environment
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// List the units in this project
    List,
    /// Show the Terraform outputs of an environment
    Output {
        /// The ID of the environment to get outputs from
        #[arg(long)]
        env: String,

        /// The name of the unit to get outputs from (optional - gets all units if not provided)
        #[arg(short = 'U', long)]
        unit: Option<String>,

        /// Save output to a file
        #[arg(short = 'f', long)]
        file: Option<PathBuf>,

        /// Output format (json or table)
        #[arg(long, default_value = "table")]
        format: String,

        /// Print detailed output during execution
        #[arg(long)]
        verbose: bool,
    },
    /// Remove the files Envie generates into units
    Clean {
        /// The name of the unit to clean
        #[arg(long)]
        unit: Option<String>,

        /// Also remove .terraform directories and provider lock files
        #[arg(long)]
        deep: bool,

        /// Print detailed output during execution
        #[arg(long)]
        verbose: bool,
    },
    /// Show detailed information about units and dependencies
    Show {
        /// The name of the unit to show (optional - shows all if not provided)
        #[arg(long)]
        unit: Option<String>,

        /// Show only sub-unit information
        #[arg(long)]
        modules: bool,

        /// Show only dependency information
        #[arg(long)]
        dependencies: bool,

        /// Print detailed output during execution
        #[arg(long)]
        verbose: bool,
    },
}
