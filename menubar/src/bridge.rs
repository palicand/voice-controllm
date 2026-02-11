use std::sync::mpsc;
use std::time::Duration;

use tao::event_loop::EventLoopProxy;
use voice_controllm_proto::event::Event as EventType;
use voice_controllm_proto::init_progress::Progress;
use voice_controllm_proto::{Empty, State as ProtoState, status::Status as StatusVariant};

use crate::client;
use crate::paths;
use crate::state::AppState;

/// Events sent from the async runtime to the GUI thread.
#[derive(Debug, Clone)]
pub enum AppEvent {
    StateChanged(AppState),
}

/// Commands sent from the GUI thread to the async runtime.
#[derive(Debug)]
pub enum Command {
    StartListening,
    StopListening,
    Shutdown,
}

/// User events for the tao event loop.
pub enum UserEvent {
    #[allow(dead_code)]
    TrayIcon(tray_icon::TrayIconEvent),
    Menu(tray_icon::menu::MenuEvent),
    App(AppEvent),
}

const RECONNECT_INTERVAL: Duration = Duration::from_secs(2);
const DAEMON_POLL_INTERVAL: Duration = Duration::from_millis(100);
const DAEMON_POLL_ATTEMPTS: usize = 50;

/// Spawn the tokio async runtime on a background thread.
/// Returns a sender for GUI -> async commands.
pub fn spawn_async_runtime(event_proxy: EventLoopProxy<UserEvent>) -> mpsc::Sender<Command> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<Command>();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to create tokio runtime");

        rt.block_on(async_main(event_proxy, cmd_rx));
    });

    cmd_tx
}

async fn async_main(event_proxy: EventLoopProxy<UserEvent>, cmd_rx: mpsc::Receiver<Command>) {
    let socket_path = match paths::socket_path() {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("Failed to determine socket path: {e}");
            send_state(&event_proxy, AppState::Error(format!("Path error: {e}")));
            return;
        }
    };

    // Spawn daemon if not running
    if !client::is_daemon_running(&socket_path).await {
        if let Err(e) = spawn_daemon() {
            tracing::error!("Failed to spawn daemon: {e}");
            send_state(&event_proxy, AppState::Error(format!("Spawn failed: {e}")));
            return;
        }

        // Wait for daemon to be ready
        let mut connected = false;
        for _ in 0..DAEMON_POLL_ATTEMPTS {
            tokio::time::sleep(DAEMON_POLL_INTERVAL).await;
            if client::is_daemon_running(&socket_path).await {
                connected = true;
                break;
            }
        }
        if !connected {
            send_state(
                &event_proxy,
                AppState::Error("Daemon failed to start".into()),
            );
            return;
        }
    }

    // Main connection loop (reconnects on disconnect)
    loop {
        match run_connected(&socket_path, &event_proxy, &cmd_rx).await {
            ConnectionResult::Shutdown => break,
            ConnectionResult::Disconnected => {
                send_state(&event_proxy, AppState::Disconnected);
                tokio::time::sleep(RECONNECT_INTERVAL).await;
            }
        }
    }
}

enum ConnectionResult {
    Shutdown,
    Disconnected,
}

async fn run_connected(
    socket_path: &std::path::PathBuf,
    event_proxy: &EventLoopProxy<UserEvent>,
    cmd_rx: &mpsc::Receiver<Command>,
) -> ConnectionResult {
    let mut grpc_client = match client::connect(socket_path).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to connect: {e}");
            return ConnectionResult::Disconnected;
        }
    };

    // Get initial state
    if let Ok(status) = grpc_client.get_status(Empty {}).await {
        let state = status_to_app_state(status.into_inner());
        send_state(event_proxy, state);
    }

    // Subscribe to events
    let mut stream = match client::subscribe(&mut grpc_client).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("Failed to subscribe: {e}");
            return ConnectionResult::Disconnected;
        }
    };

    // Event + command processing loop
    loop {
        // Check for GUI commands (non-blocking)
        match cmd_rx.try_recv() {
            Ok(Command::Shutdown) => {
                let _ = grpc_client.shutdown(Empty {}).await;
                return ConnectionResult::Shutdown;
            }
            Ok(Command::StartListening) => {
                let _ = grpc_client.start_listening(Empty {}).await;
            }
            Ok(Command::StopListening) => {
                let _ = grpc_client.stop_listening(Empty {}).await;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                return ConnectionResult::Shutdown;
            }
        }

        // Check for daemon events (with timeout so we can poll commands)
        match tokio::time::timeout(Duration::from_millis(50), stream.message()).await {
            Ok(Ok(Some(event))) => {
                if let Some(new_state) = process_event(event) {
                    send_state(event_proxy, new_state);
                }
            }
            Ok(Ok(None)) | Ok(Err(_)) => {
                return ConnectionResult::Disconnected;
            }
            Err(_) => {
                // Timeout - continue loop to check commands
            }
        }
    }
}

fn process_event(event: voice_controllm_proto::Event) -> Option<AppState> {
    match event.event? {
        EventType::StateChange(sc) => match sc.status? {
            voice_controllm_proto::state_change::Status::NewState(s) => {
                let state = ProtoState::try_from(s).ok()?;
                Some(AppState::from_proto(state))
            }
            voice_controllm_proto::state_change::Status::Error(e) => {
                Some(AppState::Error(e.message))
            }
        },
        EventType::InitProgress(progress) => match progress.progress? {
            Progress::ModelDownload(dl) => {
                let pct = if dl.bytes_total > 0 {
                    (dl.bytes_downloaded as f64 / dl.bytes_total as f64 * 100.0) as u32
                } else {
                    0
                };
                Some(AppState::Initializing {
                    message: format!("Downloading {}... {}%", dl.model_name, pct),
                })
            }
            Progress::ModelLoad(load) => Some(AppState::Initializing {
                message: format!("Loading {}...", load.model_name),
            }),
            Progress::Ready(_) => Some(AppState::Paused),
        },
        EventType::DaemonError(err) => Some(AppState::Error(err.message)),
        EventType::Transcription(_) => None,
    }
}

fn status_to_app_state(status: voice_controllm_proto::Status) -> AppState {
    match status.status {
        Some(StatusVariant::Healthy(h)) => {
            let state = ProtoState::try_from(h.state).unwrap_or(ProtoState::Stopped);
            AppState::from_proto(state)
        }
        Some(StatusVariant::Error(e)) => AppState::Error(e.message),
        None => AppState::Disconnected,
    }
}

fn send_state(proxy: &EventLoopProxy<UserEvent>, state: AppState) {
    let _ = proxy.send_event(UserEvent::App(AppEvent::StateChanged(state)));
}

fn spawn_daemon() -> anyhow::Result<()> {
    let daemon_path = std::env::current_exe()?
        .parent()
        .ok_or_else(|| anyhow::anyhow!("No parent directory"))?
        .join("voice-controllm-daemon");

    if !daemon_path.exists() {
        anyhow::bail!("Daemon binary not found at: {}", daemon_path.display());
    }

    std::process::Command::new(&daemon_path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;

    Ok(())
}
