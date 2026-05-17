pub enum MicrophoneStatus {
    Authorized,
    Denied,
    Pending,
    NotSupported,
}

#[cfg(target_os = "macos")]
pub use crate::macos::microphone::request_or_status;

#[cfg(not(target_os = "macos"))]
pub fn request_or_status() -> MicrophoneStatus {
    MicrophoneStatus::NotSupported
}
