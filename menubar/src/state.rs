use voice_controllm_proto::State as ProtoState;

/// Application state derived from daemon status.
#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    /// Not connected to daemon.
    Disconnected,
    /// Daemon is loading models.
    Initializing {
        /// Progress message to display (e.g., "Downloading model... 45%")
        message: String,
    },
    /// Connected, not listening.
    Paused,
    /// Actively capturing and transcribing.
    Listening,
    /// Daemon reported an error.
    Error(String),
}

impl AppState {
    /// Convert from proto State enum.
    pub fn from_proto(state: ProtoState) -> Self {
        match state {
            ProtoState::Stopped => AppState::Disconnected,
            ProtoState::Listening => AppState::Listening,
            ProtoState::Paused => AppState::Paused,
            ProtoState::Initializing => AppState::Initializing {
                message: "Initializing...".to_string(),
            },
        }
    }

    /// Status text shown as a disabled menu item.
    pub fn status_text(&self) -> &str {
        match self {
            AppState::Disconnected => "Disconnected",
            AppState::Initializing { message } => message,
            AppState::Paused => "Paused",
            AppState::Listening => "Listening",
            AppState::Error(msg) => msg,
        }
    }

    /// Whether the toggle action item should be shown.
    pub fn has_toggle(&self) -> bool {
        matches!(self, AppState::Paused | AppState::Listening)
    }

    /// Label for the toggle action item.
    pub fn toggle_label(&self) -> &str {
        match self {
            AppState::Listening => "Pause Listening",
            AppState::Paused => "Start Listening",
            _ => "",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_proto_listening() {
        assert_eq!(
            AppState::from_proto(ProtoState::Listening),
            AppState::Listening
        );
    }

    #[test]
    fn test_from_proto_paused() {
        assert_eq!(AppState::from_proto(ProtoState::Paused), AppState::Paused);
    }

    #[test]
    fn test_from_proto_initializing() {
        let state = AppState::from_proto(ProtoState::Initializing);
        assert!(matches!(state, AppState::Initializing { .. }));
    }

    #[test]
    fn test_from_proto_stopped_maps_to_disconnected() {
        assert_eq!(
            AppState::from_proto(ProtoState::Stopped),
            AppState::Disconnected
        );
    }

    #[test]
    fn test_status_text() {
        assert_eq!(AppState::Listening.status_text(), "Listening");
        assert_eq!(AppState::Paused.status_text(), "Paused");
        assert_eq!(AppState::Disconnected.status_text(), "Disconnected");
    }

    #[test]
    fn test_toggle_visibility() {
        assert!(AppState::Listening.has_toggle());
        assert!(AppState::Paused.has_toggle());
        assert!(!AppState::Disconnected.has_toggle());
        assert!(
            !AppState::Initializing {
                message: String::new()
            }
            .has_toggle()
        );
        assert!(!AppState::Error(String::new()).has_toggle());
    }

    #[test]
    fn test_toggle_labels() {
        assert_eq!(AppState::Listening.toggle_label(), "Pause Listening");
        assert_eq!(AppState::Paused.toggle_label(), "Start Listening");
    }
}
