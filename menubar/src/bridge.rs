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
    InstallCompleted,
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
                let proxy = event_proxy.clone();
                tokio::spawn(async move {
                    run_install_flow(script, vcmctl, proxy).await;
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

fn applescript_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// In a bundle, the script lives at `Contents/Resources/install-vcmctl-cli.sh`.
/// In dev, walk up ancestors to find the workspace `target/` directory and use
/// its parent as the workspace root — handles both `target/<profile>/vcm` and
/// `target/<triple>/<profile>/vcm` layouts.
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
            .ancestors()
            .find(|p| p.file_name().is_some_and(|n| n == "target"))
            .and_then(std::path::Path::parent)
            .map(|root| root.join("scripts").join("install-vcmctl-cli.sh"))
            .unwrap_or_else(|| parent.join("install-vcmctl-cli.sh"))
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ShellAdvice {
    export_line: String,
    config_file: String,
}

/// Map the user's `$SHELL` to the right "add `~/.local/bin` to PATH" snippet
/// and the file they should paste it into.
fn shell_advice(shell: &str) -> ShellAdvice {
    let basename = std::path::Path::new(shell)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let export_path = r#"export PATH="$HOME/.local/bin:$PATH""#.to_string();
    match basename {
        "zsh" => ShellAdvice {
            export_line: export_path,
            config_file: "~/.zshrc".to_string(),
        },
        "bash" => ShellAdvice {
            export_line: export_path,
            config_file: "~/.bash_profile".to_string(),
        },
        "fish" => ShellAdvice {
            export_line: "fish_add_path ~/.local/bin".to_string(),
            config_file: "~/.config/fish/config.fish".to_string(),
        },
        _ => ShellAdvice {
            export_line: export_path,
            config_file: "your shell's startup file (e.g. ~/.zshrc)".to_string(),
        },
    }
}

async fn run_install_flow(
    script: std::path::PathBuf,
    vcmctl: std::path::PathBuf,
    proxy: EventLoopProxy<UserEvent>,
) {
    let install_status = tokio::process::Command::new("bash")
        .arg(&script)
        .arg(&vcmctl)
        .status()
        .await;
    let status = match install_status {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(?e, "vcmctl install command failed to spawn");
            return;
        }
    };
    if !status.success() {
        tracing::warn!(?status, "vcmctl install failed");
        return;
    }
    tracing::info!("vcmctl installed at ~/.local/bin/vcmctl");

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let on_path = tokio::process::Command::new(&shell)
        .arg("-l")
        .arg("-c")
        .arg("command -v vcmctl")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false);

    if !on_path {
        let advice = shell_advice(&shell);
        if let Err(e) = copy_to_clipboard(&advice.export_line).await {
            tracing::warn!(?e, "failed to copy export line to clipboard");
        }
        if let Err(e) = show_path_advice_dialog(&advice).await {
            tracing::warn!(?e, "failed to show PATH advice dialog");
        }
    }

    let _ = proxy.send_event(UserEvent::App(AppEvent::InstallCompleted));
}

async fn copy_to_clipboard(line: &str) -> std::io::Result<()> {
    use tokio::io::AsyncWriteExt;
    let mut child = tokio::process::Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(line.as_bytes()).await?;
    }
    let status = child.wait().await?;
    if !status.success() {
        return Err(std::io::Error::other(format!(
            "pbcopy exited with status {status}"
        )));
    }
    Ok(())
}

async fn show_path_advice_dialog(advice: &ShellAdvice) -> std::io::Result<()> {
    let body = format!(
        "vcmctl was installed at ~/.local/bin/vcmctl, but that directory isn't on your PATH yet.\n\nThe command has been copied to your clipboard:\n\n    {}\n\nPaste it into {} and restart your terminal.",
        advice.export_line, advice.config_file
    );
    let applescript = format!(
        "display dialog {} buttons {{\"OK\"}} default button \"OK\" with icon note with title \"VCM\"",
        applescript_quote(&body)
    );
    let status = tokio::process::Command::new("osascript")
        .arg("-e")
        .arg(&applescript)
        .status()
        .await?;
    if !status.success() {
        return Err(std::io::Error::other(format!(
            "osascript exited with status {status}"
        )));
    }
    Ok(())
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
    fn dev_script_with_target_triple_build_dir() {
        let exe = PathBuf::from("/Users/x/proj/target/aarch64-apple-darwin/release/vcm");
        assert_eq!(
            install_script_path(&exe),
            PathBuf::from("/Users/x/proj/scripts/install-vcmctl-cli.sh")
        );
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

    #[test]
    fn shell_advice_zsh() {
        let a = shell_advice("/bin/zsh");
        assert_eq!(a.export_line, r#"export PATH="$HOME/.local/bin:$PATH""#);
        assert_eq!(a.config_file, "~/.zshrc");
    }

    #[test]
    fn shell_advice_bash() {
        let a = shell_advice("/bin/bash");
        assert_eq!(a.export_line, r#"export PATH="$HOME/.local/bin:$PATH""#);
        assert_eq!(a.config_file, "~/.bash_profile");
    }

    #[test]
    fn shell_advice_fish() {
        let a = shell_advice("/opt/homebrew/bin/fish");
        assert_eq!(a.export_line, "fish_add_path ~/.local/bin");
        assert_eq!(a.config_file, "~/.config/fish/config.fish");
    }

    #[test]
    fn shell_advice_unknown_defaults_to_zsh_line_with_generic_file() {
        let a = shell_advice("/usr/bin/something-weird");
        assert_eq!(a.export_line, r#"export PATH="$HOME/.local/bin:$PATH""#);
        assert!(
            a.config_file.contains("startup file"),
            "expected generic guidance, got {:?}",
            a.config_file
        );
    }

    #[test]
    fn shell_advice_empty_defaults_to_generic() {
        let a = shell_advice("");
        assert!(!a.export_line.is_empty());
        assert!(a.config_file.contains("startup file"));
    }
}
