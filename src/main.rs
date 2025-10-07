use clap::Parser;
use env_logger;
use log;

mod commands;
mod common;
mod cli;

use cli::args::Cli;
use cli::handler::CommandHandler;

#[tokio::main]
async fn main() {
    // Initialize logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    // Parse command line arguments
    let cli = Cli::parse();

    // Create command handler
    let handler = CommandHandler::new();

    // Handle the command
    if let Err(e) = handler.handle_command(cli.command).await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}