use clap::{Parser, Subcommand, ValueEnum};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};
use voice_controllm_daemon::config::{Config, SpeechModel};

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
