pub enum MicrophoneStatus {
    Authorized,
    Denied,
    Pending,
    NotSupported,
}

#[cfg(target_os = "macos")]
pub fn request_or_status() -> MicrophoneStatus {
    match crate::macos::microphone::request_or_status() {
        crate::macos::microphone::MicrophoneStatus::Authorized => MicrophoneStatus::Authorized,
        crate::macos::microphone::MicrophoneStatus::Denied => MicrophoneStatus::Denied,
        crate::macos::microphone::MicrophoneStatus::Pending => MicrophoneStatus::Pending,
    }
}

#[cfg(not(target_os = "macos"))]
pub fn request_or_status() -> MicrophoneStatus {
    MicrophoneStatus::NotSupported
}
