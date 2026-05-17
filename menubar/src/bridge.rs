use std::sync::mpsc;
use std::time::Duration;

use anyhow::Context as _;

use tao::event_loop::EventLoopProxy;
use vcm_proto::event::Event as EventType;
use vcm_proto::init_progress::Progress;
use vcm_proto::{Empty, SetLanguageRequest, State as ProtoState, status::Status as StatusVariant};

use vcm_common::client;
use vcm_common::dirs;

use crate::state::{AppState, LanguageInfo};

#[derive(Debug, Clone)]
pub enum AppEvent {
    StateChanged(AppState),
    LanguageChanged(LanguageInfo),
    ShutdownRequested,
    ShutdownComplete,
}

#[derive(Debug)]
pub enum Command {
    StartListening,
    StopListening,
    SetLanguage(String),
    Shutdown,
    InstallCli,
}

pub enum UserEvent {
    #[allow(dead_code)]
    TrayIcon(tray_icon::TrayIconEvent),
    Menu(tray_icon::menu::MenuEvent),
    App(AppEvent),
}

const RECONNECT_INTERVAL: Duration = Duration::from_secs(2);
const DAEMON_POLL_INTERVAL: Duration = Duration::from_millis(100);
const DAEMON_POLL_ATTEMPTS: usize = 50;

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
    let signal_proxy = event_proxy.clone();
    tokio::spawn(async move {
        use tokio::signal::unix::{SignalKind, signal};
        let (Ok(mut sigterm), Ok(mut sigint)) = (
            signal(SignalKind::terminate()),
            signal(SignalKind::interrupt()),
        ) else {
            tracing::warn!("failed to register signal handlers — graceful shutdown disabled");
            return;
        };
        tokio::select! {
            _ = sigterm.recv() => {}
            _ = sigint.recv() => {}
        }
        let _ = signal_proxy.send_event(UserEvent::App(AppEvent::ShutdownRequested));
    });

    let socket_path = match dirs::socket_path() {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("Failed to determine socket path: {e}");
            send_state(&event_proxy, AppState::Error(format!("Path error: {e}")));
            return;
        }
    };

    if !client::is_daemon_running(&socket_path).await {
        if let Err(e) = spawn_daemon() {
            tracing::error!("Failed to spawn daemon: {e}");
            send_state(&event_proxy, AppState::Error(format!("Spawn failed: {e}")));
            return;
        }

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

    loop {
        match run_connected(&socket_path, &event_proxy, &cmd_rx).await {
            ConnectionResult::Shutdown => break,
            ConnectionResult::Disconnected => {
                send_state(&event_proxy, AppState::Disconnected);
                tokio::time::sleep(RECONNECT_INTERVAL).await;
            }
        }
    }

    let _ = event_proxy.send_event(UserEvent::App(AppEvent::ShutdownComplete));
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

    if let Ok(status) = grpc_client.get_status(Empty {}).await {
        let state = status_to_app_state(status.into_inner());
        send_state(event_proxy, state);
    }

    if let Ok(resp) = grpc_client.get_language(Empty {}).await {
        send_language(event_proxy, resp.into_inner());
    }

    let mut stream = match client::subscribe(&mut grpc_client).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("Failed to subscribe: {e}");
            return ConnectionResult::Disconnected;
        }
    };

    loop {
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
            Ok(Command::SetLanguage(lang)) => {
                if grpc_client
                    .set_language(SetLanguageRequest { language: lang })
                    .await
                    .is_ok()
                    && let Ok(resp) = grpc_client.get_language(Empty {}).await
                {
                    send_language(event_proxy, resp.into_inner());
                }
            }
            Ok(Command::InstallCli) => {
                let current_exe = match std::env::current_exe() {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::error!(?e, "failed to get current_exe for InstallCli");
                        continue;
                    }
                };
                let vcmctl = vcm_common::bundle::resolve(&current_exe, vcm_common::bundle::VCMCTL);
                let script = install_script_path(&current_exe);

                let applescript = format!(
                    r#"do shell script {} with administrator privileges"#,
                    applescript_quote(&format!(
                        "{} {}",
                        shell_quote(&script.to_string_lossy()),
                        shell_quote(&vcmctl.to_string_lossy()),
                    )),
                );

                tokio::task::spawn_blocking(move || {
                    let result = std::process::Command::new("osascript")
                        .arg("-e")
                        .arg(&applescript)
                        .status();
                    match result {
                        Ok(status) if status.success() => {
                            tracing::info!("vcmctl installed in PATH");
                        }
                        Ok(status) => tracing::warn!(?status, "vcmctl install failed"),
                        Err(e) => tracing::error!(?e, "vcmctl install command failed to spawn"),
                    }
                });
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
            Err(_) => {}
        }
    }
}

fn process_event(event: vcm_proto::Event) -> Option<AppState> {
    match event.event? {
        EventType::StateChange(sc) => match sc.status? {
            vcm_proto::state_change::Status::NewState(s) => {
                let state = ProtoState::try_from(s).ok()?;
                Some(AppState::from_proto(state))
            }
            vcm_proto::state_change::Status::Error(e) => Some(AppState::Error(e.message)),
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

fn status_to_app_state(status: vcm_proto::Status) -> AppState {
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

fn send_language(proxy: &EventLoopProxy<UserEvent>, resp: vcm_proto::GetLanguageResponse) {
    use crate::state::LanguageSelection;

    let active = if resp.language.is_empty() || resp.language.eq_ignore_ascii_case("auto") {
        LanguageSelection::Auto
    } else {
        LanguageSelection::Fixed(resp.language)
    };
    let info = LanguageInfo {
        active,
        available: resp.available_languages,
    };
    let _ = proxy.send_event(UserEvent::App(AppEvent::LanguageChanged(info)));
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

fn applescript_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// In a bundle, the script lives at `Contents/Resources/install-vcmctl-cli.sh`.
/// In dev (`target/<profile>/vcm`), walk up two levels to the workspace root.
fn install_script_path(current_exe: &std::path::Path) -> std::path::PathBuf {
    let parent = current_exe
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default();

    if parent.ends_with("Contents/MacOS") {
        parent
            .parent()
            .map(|contents| contents.join("Resources").join("install-vcmctl-cli.sh"))
            .unwrap_or_else(|| parent.join("install-vcmctl-cli.sh"))
    } else {
        parent
            .parent()
            .and_then(|p| p.parent())
            .map(|root| root.join("scripts").join("install-vcmctl-cli.sh"))
            .unwrap_or_else(|| parent.join("install-vcmctl-cli.sh"))
    }
}

fn spawn_daemon() -> anyhow::Result<()> {
    let current_exe = std::env::current_exe().context("get current exe")?;
    let daemon_path = vcm_common::bundle::resolve(&current_exe, vcm_common::bundle::VCMD);

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn bundled_script_in_resources() {
        let exe = PathBuf::from("/Applications/VCM.app/Contents/MacOS/vcm");
        assert_eq!(
            install_script_path(&exe),
            PathBuf::from("/Applications/VCM.app/Contents/Resources/install-vcmctl-cli.sh")
        );
    }

    #[test]
    fn dev_script_in_workspace_scripts_dir() {
        let exe = PathBuf::from("/Users/x/proj/target/debug/vcm");
        assert_eq!(
            install_script_path(&exe),
            PathBuf::from("/Users/x/proj/scripts/install-vcmctl-cli.sh")
        );
    }

    #[test]
    fn shell_quote_simple() {
        assert_eq!(shell_quote("foo"), "'foo'");
    }

    #[test]
    fn shell_quote_with_space() {
        assert_eq!(shell_quote("/Users/A B/bin/x"), "'/Users/A B/bin/x'");
    }

    #[test]
    fn shell_quote_with_single_quote() {
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn applescript_quote_simple() {
        assert_eq!(applescript_quote("foo"), "\"foo\"");
    }

    #[test]
    fn applescript_quote_with_double_quote() {
        assert_eq!(applescript_quote(r#"he said "hi""#), r#""he said \"hi\"""#);
    }

    #[test]
    fn applescript_quote_with_backslash() {
        assert_eq!(applescript_quote(r"a\b"), r#""a\\b""#);
    }
}
