use clap::Parser;
use envie::cli::args::Cli;
use envie::cli::handler::CommandHandler;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    let cli = Cli::parse();

    if let Err(e) = CommandHandler::new().handle_command(cli.command).await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
