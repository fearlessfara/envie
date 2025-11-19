use clap::{Parser, Subcommand};
use std::path::PathBuf;

const ENVIE_LONG_ABOUT: &str = "\
A tool for managing multiple ephemeral environments in Terraform with layered dependencies and resource sharing.

QUICK START:
    envie init --name myapp                    # Initialize new project
    envie deploy --unit api --env dev-123      # Deploy to ephemeral environment
    envie plan --unit api --env dev-123        # Preview deployment
    envie list                                 # List all units

For detailed help on any command, run:
    envie <command> --help
";

#[derive(Parser)]
#[command(name = "envie")]
#[command(about = "A tool for managing multiple ephemeral environments in Terraform with layered dependencies and resource sharing")]
#[command(long_about = ENVIE_LONG_ABOUT)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize workspace or unit configuration
    #[command(after_help = "\
EXAMPLES:
    # Initialize workspace (creates workspace.envie.yaml)
    envie init --project

    # Initialize unit (creates envie.yaml in folder)
    envie init --unit services/auth

    # Initialize current directory as workspace
    envie init --project --name myapp
    ")]
    Init {
        /// Initialize workspace configuration
        #[arg(long, conflicts_with = "unit")]
        project: bool,

        /// Initialize unit at path (creates folder if needed)
        #[arg(long, conflicts_with = "project")]
        unit: Option<String>,

        /// Project/unit name
        #[arg(long)]
        name: Option<String>,

        /// Project description (workspace only)
        #[arg(long)]
        description: Option<String>,

        /// Print detailed output during execution
        #[arg(long)]
        verbose: bool,
    },
    /// Deploy a unit with dependency management and Terraform orchestration
    #[command(after_help = "\
EXAMPLES:
    # Deploy a single unit to ephemeral environment
    envie deploy --unit api --env dev-123

    # Deploy with environment overrides (use stable database)
    envie deploy --unit api --env feature-branch \\
      -E database:stable.sandbox \\
      -E networking:stable.sandbox

    # Deploy from within a unit directory (auto-discovery)
    cd services/api
    envie deploy --env my-test

    # Preview deployment without applying changes
    envie deploy --unit api --env test --dry-run

    # Deploy with verbose environment resolution
    envie deploy --unit api --env dev-123 --verbose

    # Deploy all units under a path
    envie deploy --unit services/api --env integration-test

TIP: Use 'envie plan' as a shortcut for '--dry-run'
    ")]
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
    /// Preview deployment without making changes (alias for deploy --dry-run)
    #[command(after_help = "\
EXAMPLES:
    # Preview what will be deployed
    envie plan --unit api --env dev-123

    # Preview with environment overrides
    envie plan --unit api --env test \\
      -E database:stable.production

    # Preview with verbose environment resolution
    envie plan --unit api --env feature-branch --verbose

NOTE: This is equivalent to 'envie deploy --dry-run'
    ")]
    Plan {
        /// The name of the unit to preview (optional - will auto-discover from current directory)
        #[arg(short = 'U', long)]
        unit: Option<String>,

        /// The ID of the environment to preview
        #[arg(long)]
        env: String,

        /// Override environment for specific dependencies (format: unit:environment)
        #[arg(short = 'E', long, action = clap::ArgAction::Append)]
        environment: Vec<String>,

        /// Print detailed output during execution
        #[arg(long)]
        verbose: bool,
    },
    /// Destroy the environment for a specific unit
    #[command(after_help = "\
EXAMPLES:
    # Destroy resources in an environment
    envie destroy --unit api --env dev-123

    # Preview what will be destroyed (dry run)
    envie destroy --unit api --env test --dry-run

    # Destroy with verbose output
    envie destroy --unit api --env feature-branch --verbose

NOTE: This keeps the backend infrastructure (S3/DynamoDB).
      Use 'envie delete' for complete cleanup.
    ")]
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
    /// Refresh state to match reality (like terraform refresh)
    #[command(after_help = "\
EXAMPLES:
    # Refresh state for a unit
    envie refresh --unit api --env dev-123

    # Refresh with verbose output
    envie refresh --unit api --env test --verbose
    ")]
    Refresh {
        /// The name of the unit to refresh (optional - will auto-discover from current directory)
        #[arg(short = 'U', long)]
        unit: Option<String>,

        /// The ID of the environment
        #[arg(long)]
        env: String,

        /// Print detailed output during execution
        #[arg(long)]
        verbose: bool,
    },
    /// Completely delete an environment including state management infrastructure
    #[command(after_help = "\
EXAMPLES:
    # Delete an entire environment (with confirmation)
    envie delete --env dev-123

    # Delete without confirmation prompt
    envie delete --env test --no-prompt

    # Preview what will be deleted (dry run)
    envie delete --env feature-branch --dry-run

WARNING: This completely removes the environment including backend state!
         Use 'envie destroy' if you want to keep state infrastructure.
    ")]
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
    #[command(after_help = "\
EXAMPLES:
    # Generate .env from .env.example template
    envie generate

    # Use a custom template file
    envie generate --env-file .env.template

    # Use existing outputs.json file
    envie generate --file outputs.json

WORKFLOW:
    1. Create .env.example with placeholders
    2. Run 'envie output --env dev-123 --format json --file outputs.json'
    3. Run 'envie generate --file outputs.json'
    ")]
    Generate {
        /// Path to the environment file template
        #[arg(long, default_value = ".env.example")]
        env_file: PathBuf,
        
        /// Path to the Terraform output file (instead of calling envie output)
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// List all available development environments
    #[command(after_help = "\
EXAMPLES:
    # List all discovered units and their workspaces
    envie list

OUTPUT:
    Shows all units with their active workspaces, making it easy to see
    what's deployed and where.
    ")]
    List,
    /// Generate combined outputs for all units
    #[command(after_help = "\
EXAMPLES:
    # Get outputs for all units in an environment
    envie output --env dev-123

    # Get outputs for a specific unit
    envie output --env dev-123 --unit api

    # Save outputs to a JSON file
    envie output --env dev-123 --format json --file outputs.json

    # Get outputs in table format (default)
    envie output --env dev-123 --format table

TIP: Use the JSON output with 'envie generate' to create .env files
    ")]
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
    /// Clean .terraform directories and reinitialize Terraform
    #[command(after_help = "\
EXAMPLES:
    # Clean and reinitialize a specific unit
    envie clean --unit api

    # Clean all units (runs from project root)
    envie clean

    # Clean and upgrade providers
    envie clean --unit api --upgrade

TIP: Use this when switching between environments or after updating providers
    ")]
    Clean {
        /// The name of the unit to clean
        #[arg(long)]
        unit: Option<String>,
        
        /// Run 'terraform init -upgrade' instead of 'terraform init'
        #[arg(long)]
        upgrade: bool,
        
        /// Print detailed output during execution
        #[arg(long)]
        verbose: bool,
    },
    /// Show detailed information about units and dependencies
    #[command(after_help = "\
EXAMPLES:
    # Show information about a specific unit
    envie show --unit api

    # Show all units
    envie show

    # Show only dependencies
    envie show --unit api --dependencies

    # Show only sub-units/modules
    envie show --unit api --modules

    # Show with verbose output
    envie show --unit api --verbose
    ")]
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
    /// Run health checks on your Envie project and environment
    #[command(after_help = "\
EXAMPLES:
    # Run all health checks
    envie doctor

    # Run checks with verbose output
    envie doctor --verbose

CHECKS PERFORMED:
    • Prerequisites (Terraform, AWS CLI, Git)
    • Project configuration validity
    • Unit discovery and validation
    • AWS resource accessibility
    • Dependency graph integrity

TIP: Run this after initial setup or when troubleshooting issues
    ")]
    Doctor {
        /// Print detailed output during checks
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