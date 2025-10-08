use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "envie")]
#[command(about = "A tool for managing multiple ephemeral environments in Terraform with layered dependencies and resource sharing")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new Envie project with configuration scaffolding
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
    /// Destroy the environment for a specific unit
    Destroy {
        /// The name of the unit to destroy (optional - will auto-discover from current directory)
        #[arg(short = 'U', long)]
        unit: Option<String>,

        /// The ID of the environment to destroy
        #[arg(long)]
        env: Option<String>,

        /// Simulate the destruction process without making changes
        #[arg(short = 'D', long)]
        dry_run: bool,

        /// Print detailed output during execution
        #[arg(long)]
        verbose: bool,
    },
    /// Completely delete an environment including state management infrastructure
    Delete {
        /// The name of the unit to delete (optional - deletes all units if not specified)
        #[arg(short = 'U', long)]
        unit: Option<String>,

        /// The ID of the environment to delete
        #[arg(long)]
        env: String,

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
    /// Manage ephemeral development environments
    Env {
        #[command(subcommand)]
        command: EnvCommands,
    },
    /// Generate environment variables from Terraform outputs
    Generate {
        /// Path to the environment file template
        #[arg(long, default_value = ".env.example")]
        env_file: PathBuf,
        
        /// Path to the Terraform output file (instead of calling envie output)
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// List all available development environments
    List,
    /// Generate combined outputs for all services and components
    Output {
        /// Save output to a file
        #[arg(short = 'f', long)]
        file: Option<PathBuf>,
        
        /// Print detailed output during execution
        #[arg(long)]
        verbose: bool,
    },
    /// Clean .terraform directories and reinitialize Terraform
    Clean {
        /// The name of the service to clean
        #[arg(long)]
        service: Option<String>,
        
        /// Run 'terraform init -upgrade' instead of 'terraform init'
        #[arg(long)]
        upgrade: bool,
        
        /// Print detailed output during execution
        #[arg(long)]
        verbose: bool,
    },
    /// Show detailed information about services, modules, and dependencies
    Show {
        /// The name of the service to show (optional - shows all if not provided)
        #[arg(long)]
        service: Option<String>,
        
        /// Show only module information
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

#[derive(Subcommand)]
pub enum EnvCommands {
    /// Start a new ephemeral dev environment
    Start {
        /// The ID of the environment
        env_id: String,
        
        /// Run commands silently without displaying output
        #[arg(long)]
        quiet: bool,
    },
    /// Destroy the specified or current active development environment
    Destroy {
        /// The ID of the environment (optional)
        env_id: Option<String>,
        
        /// Run commands silently without displaying output
        #[arg(long)]
        quiet: bool,
    },
    /// List all available development environments
    List,
    /// Display the current active development environment
    Current,
    /// Test the flexible unit discovery system
    TestDiscovery,
}