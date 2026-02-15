use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};
use voice_controllm_common::client;
use voice_controllm_common::dirs::socket_path;
use voice_controllm_daemon::config::{Config, SpeechModel};
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
        let pid_path = voice_controllm_daemon::socket::pid_path()?;
        let pid = std::fs::read_to_string(&pid_path).unwrap_or_else(|_| "unknown".to_string());
        println!("Daemon already running (PID: {})", pid.trim());
        return Ok(());
    }

    // Spawn daemon as detached process
    let daemon_path = std::env::current_exe()?
        .parent()
        .context("No parent directory")?
        .join("voice-controllm-daemon");

    if !daemon_path.exists() {
        anyhow::bail!("Daemon binary not found at: {}", daemon_path.display());
    }

    println!("Starting daemon...");

    std::process::Command::new(&daemon_path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("Failed to spawn daemon")?;

    // Wait for daemon to accept connections
    print!("Waiting for daemon to start...");
    use std::io::Write;
    std::io::stdout().flush().ok();

    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if client::is_daemon_running(&sock_path).await {
            break;
        }
    }
    println!();

    if !client::is_daemon_running(&sock_path).await {
        let log_path = voice_controllm_daemon::socket::log_path()?;
        eprintln!("Daemon failed to start. Check logs: {}", log_path.display());
        std::process::exit(1);
    }

    // Connect and check current state
    let mut grpc_client = client::connect(&sock_path).await?;
    wait_for_ready(&mut grpc_client).await
}

/// Poll daemon status until it leaves Initializing, showing progress from event stream.
async fn wait_for_ready(
    grpc_client: &mut voice_controllm_proto::voice_controllm_client::VoiceControllmClient<
        tonic::transport::Channel,
    >,
) -> Result<()> {
    // Check if already past initialization
    if !is_initializing(grpc_client).await? {
        print_daemon_ready()?;
        return Ok(());
    }

    println!("Initializing models...");

    // Subscribe for progress events, but poll status as fallback
    // (events sent before subscribe are missed)
    let mut stream = client::subscribe(grpc_client).await?;

    loop {
        tokio::select! {
            msg = stream.message() => {
                match msg? {
                    Some(event) => {
                        if handle_init_event(event, grpc_client).await? {
                            return Ok(());
                        }
                    }
                    None => break, // stream ended
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(2)) => {
                // Fallback: poll status in case we missed the Ready event
                if !is_initializing(grpc_client).await? {
                    print_daemon_ready()?;
                    return Ok(());
                }
            }
        }
    }

    print_daemon_ready()?;
    Ok(())
}

async fn is_initializing(
    grpc_client: &mut voice_controllm_proto::voice_controllm_client::VoiceControllmClient<
        tonic::transport::Channel,
    >,
) -> Result<bool> {
    let status = grpc_client
        .get_status(Empty {})
        .await
        .context("Failed to get status")?
        .into_inner();

    if let Some(StatusVariant::Healthy(h)) = status.status {
        let state = State::try_from(h.state).unwrap_or(State::Stopped);
        return Ok(state == State::Initializing);
    }
    Ok(false)
}

fn print_daemon_ready() -> Result<()> {
    let pid_path = voice_controllm_daemon::socket::pid_path()?;
    let pid = std::fs::read_to_string(&pid_path).unwrap_or_else(|_| "unknown".to_string());
    println!("Daemon ready (PID: {})", pid.trim());
    Ok(())
}

/// Handle a single init event. Returns true if initialization is complete.
async fn handle_init_event(
    event: voice_controllm_proto::Event,
    grpc_client: &mut voice_controllm_proto::voice_controllm_client::VoiceControllmClient<
        tonic::transport::Channel,
    >,
) -> Result<bool> {
    use voice_controllm_proto::event::Event as EventType;
    use voice_controllm_proto::init_progress::Progress;

    match event.event {
        Some(EventType::InitProgress(progress)) => match progress.progress {
            Some(Progress::ModelDownload(dl)) => {
                let mb_done = dl.bytes_downloaded as f64 / 1_000_000.0;
                let mb_total = dl.bytes_total as f64 / 1_000_000.0;
                if mb_total > 0.0 {
                    print!(
                        "\rDownloading {}... {:.0}/{:.0} MB",
                        dl.model_name, mb_done, mb_total
                    );
                } else {
                    print!("\rDownloading {}... {:.0} MB", dl.model_name, mb_done);
                }
                use std::io::Write;
                std::io::stdout().flush().ok();
            }
            Some(Progress::ModelLoad(load)) => {
                println!("Loading {}...", load.model_name);
            }
            Some(Progress::Ready(_)) => {
                print_daemon_ready()?;
                return Ok(true);
            }
            None => {}
        },
        Some(EventType::DaemonError(err)) => {
            handle_daemon_error(err, grpc_client).await?;
        }
        Some(EventType::StateChange(_) | EventType::Transcription(_)) | None => {}
    }
    Ok(false)
}

async fn handle_daemon_error(
    err: voice_controllm_proto::DaemonError,
    grpc_client: &mut voice_controllm_proto::voice_controllm_client::VoiceControllmClient<
        tonic::transport::Channel,
    >,
) -> Result<()> {
    let kind = voice_controllm_proto::ErrorKind::try_from(err.kind)
        .unwrap_or(voice_controllm_proto::ErrorKind::ErrorUnknown);
    match kind {
        voice_controllm_proto::ErrorKind::ErrorModelMissing => {
            println!("Model '{}' not found. Downloading...", err.model_name);
            grpc_client
                .download_models(Empty {})
                .await
                .context("Failed to trigger model download")?;
        }
        voice_controllm_proto::ErrorKind::ErrorModelCorrupted => {
            println!(
                "Model '{}' appears corrupted: {}. Re-downloading...",
                err.model_name, err.message
            );
            grpc_client
                .download_models(Empty {})
                .await
                .context("Failed to trigger model re-download")?;
        }
        _ => {
            eprintln!("Daemon error: {}", err.message);
            std::process::exit(1);
        }
    }
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
                State::Initializing => println!("Initializing..."),
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
                State::Initializing => {
                    println!("Daemon is still initializing, please wait...");
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
                println!("Language: {:?}", config.model.language);
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
                println!("language = {:?}", config.model.language);
                println!();
                println!("[latency]");
                println!("mode = {:?}", config.latency.mode);
                println!("min_chunk_seconds = {}", config.latency.min_chunk_seconds);
            }
        },
    }

    Ok(())
}
