//! Daemon runner that orchestrates all components.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::sync::{broadcast, oneshot};
use tonic::transport::Server;
use tracing::{error, info};
use voice_controllm_proto::{
    DaemonError, ErrorKind, Event, InitProgress, ModelDownload, ModelLoad, Ready,
};

use crate::config::Config;
use crate::controller::Controller;
use crate::engine::{Engine, InitEvent};
use crate::server::VoiceControllmService;
use crate::socket::{cleanup_socket, create_listener};

/// Paths used by the daemon at runtime.
pub struct DaemonPaths {
    pub socket: PathBuf,
    pub pid: PathBuf,
}

impl DaemonPaths {
    /// Create paths using XDG state directory defaults.
    pub fn from_xdg() -> Result<Self> {
        Ok(Self {
            socket: crate::socket::socket_path()?,
            pid: crate::socket::pid_path()?,
        })
    }
}

/// Run the daemon with default XDG paths.
pub async fn run() -> Result<()> {
    run_with_paths(DaemonPaths::from_xdg()?).await
}

/// Run the daemon with custom paths (loads config from XDG).
pub async fn run_with_paths(paths: DaemonPaths) -> Result<()> {
    let config = Config::load().context("Failed to load config")?;
    run_with_paths_and_config(paths, config).await
}

/// Run the daemon with custom paths and config (for testing).
pub async fn run_with_paths_and_config(paths: DaemonPaths, config: Config) -> Result<()> {
    let sock_path = paths.socket;
    let pid_file = paths.pid;

    info!(model = ?config.model.model, "Loaded configuration");

    // Write PID file
    let pid = std::process::id();
    std::fs::write(&pid_file, pid.to_string()).context("Failed to write PID file")?;
    info!(pid = pid, path = %pid_file.display(), "Wrote PID file");

    // Create Unix socket listener
    let listener = create_listener(&sock_path)?;
    info!(path = %sock_path.display(), "Listening on Unix socket");

    // Create shutdown channel
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    // Create event channel
    let (event_tx, _) = broadcast::channel(256);

    // Create engine
    let engine = Engine::new(config.clone()).context("Failed to create engine")?;

    // Create controller (starts in Initializing state)
    let controller = Arc::new(Controller::new(
        event_tx.clone(),
        shutdown_tx,
        engine,
        config.injection.clone(),
    ));

    // Create gRPC service
    let service = VoiceControllmService::new(controller.clone());

    // Convert UnixListener to stream
    let incoming = async_stream::stream! {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => yield Ok::<_, std::io::Error>(stream),
                Err(e) => {
                    tracing::error!(error = %e, "Accept error");
                }
            }
        }
    };

    // Spawn initialization task
    let init_controller = controller.clone();
    let init_event_tx = event_tx.clone();
    tokio::spawn(async move {
        initialize_engine(init_controller, init_event_tx).await;
    });

    // Run server with graceful shutdown
    info!("Daemon started");
    let server = Server::builder()
        .add_service(service.into_server())
        .serve_with_incoming_shutdown(incoming, async {
            let _ = shutdown_rx.await;
            info!("Shutdown signal received");
        });

    let result = server.await;

    // Cleanup
    cleanup_socket(&sock_path);
    let _ = std::fs::remove_file(&pid_file);
    info!("Daemon stopped");

    result.context("Server error")
}

/// Convert an engine InitEvent to a proto Event.
fn init_event_to_proto(event: InitEvent) -> Event {
    let progress = match event {
        InitEvent::Loading { model } => {
            voice_controllm_proto::init_progress::Progress::ModelLoad(ModelLoad {
                model_name: model,
            })
        }
        InitEvent::Downloading {
            model,
            bytes,
            total,
        } => voice_controllm_proto::init_progress::Progress::ModelDownload(ModelDownload {
            model_name: model,
            bytes_downloaded: bytes,
            bytes_total: total,
        }),
        InitEvent::Ready => voice_controllm_proto::init_progress::Progress::Ready(Ready {}),
    };

    Event {
        event: Some(voice_controllm_proto::event::Event::InitProgress(
            InitProgress {
                progress: Some(progress),
            },
        )),
    }
}

/// Broadcast an engine error as a DaemonError event.
fn engine_error_event(err: &anyhow::Error) -> Event {
    Event {
        event: Some(voice_controllm_proto::event::Event::DaemonError(
            DaemonError {
                kind: ErrorKind::ErrorEngine.into(),
                message: format!("{:#}", err),
                model_name: String::new(),
            },
        )),
    }
}

/// Initialize the engine in a background task.
async fn initialize_engine(controller: Arc<Controller>, event_tx: broadcast::Sender<Event>) {
    let mut engine = match controller.take_engine().await {
        Some(e) => e,
        None => {
            error!("No engine available for initialization");
            return;
        }
    };

    let tx = event_tx.clone();
    let result = engine
        .initialize(move |event| {
            let _ = tx.send(init_event_to_proto(event));
        })
        .await;

    controller.return_engine(engine).await;

    match result {
        Ok(()) => {
            controller.mark_ready().await;
            info!("Engine initialization complete");
        }
        Err(e) => {
            error!(error = %e, "Engine initialization failed");
            let _ = event_tx.send(engine_error_event(&e));
        }
    }
}
