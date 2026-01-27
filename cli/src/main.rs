use clap::{Parser, Subcommand};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

#[derive(Parser)]
#[command(name = "vcm")]
#[command(about = "Voice-Controllm CLI - offline voice dictation")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the voice dictation daemon
    Start,
    /// Stop the voice dictation daemon
    Stop,
    /// Show daemon status
    Status,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Start => {
            tracing::info!("Starting voice-controllm daemon...");
            println!("Starting daemon... (not yet implemented)");
        }
        Commands::Stop => {
            tracing::info!("Stopping voice-controllm daemon...");
            println!("Stopping daemon... (not yet implemented)");
        }
        Commands::Status => {
            println!("Daemon status: unknown (not yet implemented)");
        }
    }

    Ok(())
}
