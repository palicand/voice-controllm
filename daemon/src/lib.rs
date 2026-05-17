pub mod audio;
pub mod config;
pub mod controller;
pub mod daemon;
pub mod dirs;
pub mod engine;
pub mod inject;
pub mod models;
pub mod server;
pub mod socket;
pub mod transcribe;
pub mod vad;

pub async fn run() -> anyhow::Result<()> {
    let config = config::Config::load().unwrap_or_default();
    let state_dir = vcm_common::dirs::state_dir()?;

    vcm_platform::logging::init(
        vcm_platform::logging::LogCategory::Daemon,
        config.logging.level.as_directive(),
        state_dir,
    )?;

    whisper_rs::install_logging_hooks();

    if !vcm_platform::accessibility::is_trusted_or_prompt() {
        tracing::warn!(
            "Accessibility permission not granted; system prompt was shown. \
             Keystroke injection will fail until granted."
        );
    }

    match vcm_platform::microphone::request_or_status() {
        vcm_platform::microphone::MicrophoneStatus::Authorized
        | vcm_platform::microphone::MicrophoneStatus::NotSupported => {}
        vcm_platform::microphone::MicrophoneStatus::Pending => {
            tracing::warn!(
                "Microphone permission prompt shown; audio capture will start once granted."
            );
        }
        vcm_platform::microphone::MicrophoneStatus::Denied => {
            tracing::error!(
                "Microphone permission denied. Grant it in System Settings → Privacy & Security → Microphone."
            );
        }
    }

    daemon::run().await
}
