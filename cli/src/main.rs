mod client;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};
use voice_controllm_daemon::config::{Config, SpeechModel};
use voice_controllm_daemon::socket::{pid_path, socket_path};
use voice_controllm_proto::{Empty, State, status::Status as StatusVariant};

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
    /// Toggle listening on/off
    Toggle,
    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show configuration file path
    Path,
    /// Create default configuration file
    Init {
        /// Speech model to use
        #[arg(long, short, value_enum, default_value = "whisper-base")]
        model: ModelArg,
        /// Overwrite existing config file
        #[arg(long)]
        force: bool,
    },
    /// Show current configuration
    Show,
}

#[derive(Clone, ValueEnum)]
enum ModelArg {
    WhisperTiny,
    WhisperTinyEn,
    WhisperBase,
    WhisperBaseEn,
    WhisperSmall,
    WhisperSmallEn,
    WhisperMedium,
    WhisperMediumEn,
    WhisperLargeV3,
    WhisperLargeV3Turbo,
}

impl From<ModelArg> for SpeechModel {
    fn from(arg: ModelArg) -> Self {
        match arg {
            ModelArg::WhisperTiny => SpeechModel::WhisperTiny,
            ModelArg::WhisperTinyEn => SpeechModel::WhisperTinyEn,
            ModelArg::WhisperBase => SpeechModel::WhisperBase,
            ModelArg::WhisperBaseEn => SpeechModel::WhisperBaseEn,
            ModelArg::WhisperSmall => SpeechModel::WhisperSmall,
            ModelArg::WhisperSmallEn => SpeechModel::WhisperSmallEn,
            ModelArg::WhisperMedium => SpeechModel::WhisperMedium,
            ModelArg::WhisperMediumEn => SpeechModel::WhisperMediumEn,
            ModelArg::WhisperLargeV3 => SpeechModel::WhisperLargeV3,
            ModelArg::WhisperLargeV3Turbo => SpeechModel::WhisperLargeV3Turbo,
        }
    }
}

async fn cmd_start() -> Result<()> {
    let sock_path = socket_path()?;

    if client::is_daemon_running(&sock_path).await {
        let pid_file = pid_path()?;
        let pid = std::fs::read_to_string(&pid_file).unwrap_or_else(|_| "unknown".to_string());
        println!("Daemon already running (PID: {})", pid.trim());
        return Ok(());
    }

    println!("Starting daemon... (spawning not yet implemented)");
    // Task 7 will implement actual spawning
    Ok(())
}

async fn cmd_stop() -> Result<()> {
    let sock_path = socket_path()?;

    if !client::is_daemon_running(&sock_path).await {
        println!("Daemon not running");
        return Ok(());
    }

    let mut client = client::connect(&sock_path).await?;
    client
        .shutdown(Empty {})
        .await
        .context("Failed to send shutdown")?;

    println!("Daemon stopped");
    Ok(())
}

async fn cmd_status() -> Result<()> {
    let sock_path = socket_path()?;

    if !sock_path.exists() {
        println!("Daemon not running");
        return Ok(());
    }

    let mut client = match client::connect(&sock_path).await {
        Ok(c) => c,
        Err(_) => {
            println!("Daemon not running");
            return Ok(());
        }
    };

    let response = client
        .get_status(Empty {})
        .await
        .context("Failed to get status")?;

    let status = response.into_inner();
    match status.status {
        Some(StatusVariant::Healthy(h)) => {
            let state = State::try_from(h.state).unwrap_or(State::Stopped);
            match state {
                State::Stopped => println!("Stopped"),
                State::Listening => println!("Listening"),
                State::Paused => println!("Paused"),
            }
        }
        Some(StatusVariant::Error(e)) => {
            println!("Error: {}", e.message);
        }
        None => {
            println!("Unknown status");
        }
    }

    Ok(())
}

async fn cmd_toggle() -> Result<()> {
    let sock_path = socket_path()?;

    if !client::is_daemon_running(&sock_path).await {
        println!("Daemon not running");
        return Ok(());
    }

    let mut client = client::connect(&sock_path).await?;

    let response = client
        .get_status(Empty {})
        .await
        .context("Failed to get status")?;

    let status = response.into_inner();
    match status.status {
        Some(StatusVariant::Healthy(h)) => {
            let state = State::try_from(h.state).unwrap_or(State::Stopped);
            match state {
                State::Listening => {
                    client
                        .stop_listening(Empty {})
                        .await
                        .context("Failed to stop listening")?;
                    println!("Paused");
                }
                State::Paused => {
                    client
                        .start_listening(Empty {})
                        .await
                        .context("Failed to start listening")?;
                    println!("Listening");
                }
                State::Stopped => {
                    println!("Daemon is stopped");
                }
            }
        }
        Some(StatusVariant::Error(e)) => {
            println!("Error: {}", e.message);
        }
        None => {
            println!("Unknown status");
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Start => cmd_start().await?,
        Commands::Stop => cmd_stop().await?,
        Commands::Status => cmd_status().await?,
        Commands::Toggle => cmd_toggle().await?,
        Commands::Config { action } => match action {
            ConfigAction::Path => {
                let path = Config::config_path()?;
                println!("{}", path.display());
            }
            ConfigAction::Init { model, force } => {
                let path = Config::config_path()?;
                if path.exists() && !force {
                    eprintln!("Config file already exists: {}", path.display());
                    eprintln!("Use --force to overwrite");
                    std::process::exit(1);
                }

                let mut config = Config::default();
                config.model.model = model.into();
                config.save()?;

                println!("Created config file: {}", path.display());
                println!();
                println!("Model: {:?}", config.model.model);
                println!("Languages: {:?}", config.model.languages);
            }
            ConfigAction::Show => {
                let path = Config::config_path()?;
                if !path.exists() {
                    println!("No config file found at: {}", path.display());
                    println!("Using defaults. Run 'vcm config init' to create one.");
                    println!();
                }

                let config = Config::load()?;
                println!("Config path: {}", path.display());
                println!();
                println!("[model]");
                println!("model = {:?}", config.model.model);
                println!("languages = {:?}", config.model.languages);
                println!();
                println!("[latency]");
                println!("mode = {:?}", config.latency.mode);
                println!("min_chunk_seconds = {}", config.latency.min_chunk_seconds);
            }
        },
    }

    Ok(())
}
