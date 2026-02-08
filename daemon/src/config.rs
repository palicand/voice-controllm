//! Configuration management for voice-controllm daemon.
//!
//! Handles loading, saving, and providing defaults for the daemon configuration.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Main configuration struct for the daemon.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub model: ModelConfig,
    pub latency: LatencyConfig,
    pub injection: InjectionConfig,
    pub logging: LoggingConfig,
}

/// Configuration for the speech recognition model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelConfig {
    /// Speech recognition model to use.
    pub model: SpeechModel,
    /// Languages to recognize. Use ["auto"] for automatic detection.
    pub languages: Vec<String>,
}

/// Latency/accuracy trade-off configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LatencyConfig {
    /// Latency mode: "fast", "balanced", or "accurate".
    pub mode: LatencyMode,
    /// Minimum chunk duration in seconds before transcription.
    pub min_chunk_seconds: f32,
}

/// Latency mode enum for transcription timing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LatencyMode {
    Fast,
    #[default]
    Balanced,
    Accurate,
}

/// Supported speech recognition models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SpeechModel {
    // Whisper models (OpenAI) - via whisper.cpp
    WhisperTiny,
    WhisperTinyEn,
    #[default]
    WhisperBase,
    WhisperBaseEn,
    WhisperSmall,
    WhisperSmallEn,
    WhisperMedium,
    WhisperMediumEn,
    WhisperLargeV3,
    WhisperLargeV3Turbo,
}

/// Configuration for keystroke injection behavior.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InjectionConfig {
    /// List of application names to inject into. Empty means inject into all apps.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowlist: Vec<String>,
}

/// Logging configuration.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    /// Log level: "error", "warn", "info", "debug", "trace".
    pub level: LogLevel,
}

/// Log verbosity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    /// Convert to a tracing filter directive string for the daemon crate.
    pub fn as_directive(&self) -> &'static str {
        match self {
            LogLevel::Error => "voice_controllm_daemon=error",
            LogLevel::Warn => "voice_controllm_daemon=warn",
            LogLevel::Info => "voice_controllm_daemon=info",
            LogLevel::Debug => "voice_controllm_daemon=debug",
            LogLevel::Trace => "voice_controllm_daemon=trace",
        }
    }
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            model: SpeechModel::default(),
            languages: vec!["auto".to_string()],
        }
    }
}

impl Default for LatencyConfig {
    fn default() -> Self {
        Self {
            mode: LatencyMode::Balanced,
            min_chunk_seconds: 1.0,
        }
    }
}

impl Config {
    /// Returns the default config directory path.
    /// `~/.config/voice-controllm/` (or `$XDG_CONFIG_HOME/voice-controllm/`)
    pub fn config_dir() -> Result<PathBuf> {
        crate::dirs::config_dir()
    }

    /// Returns the default config file path.
    /// `~/.config/voice-controllm/config.toml`
    pub fn config_path() -> Result<PathBuf> {
        Self::config_dir().map(|p| p.join("config.toml"))
    }

    /// Returns the default data directory path.
    /// `~/.local/share/voice-controllm/` (or `$XDG_DATA_HOME/voice-controllm/`)
    pub fn data_dir() -> Result<PathBuf> {
        crate::dirs::data_dir()
    }

    /// Returns the default models directory path.
    /// `~/.local/share/voice-controllm/models/`
    pub fn models_dir() -> Result<PathBuf> {
        Self::data_dir().map(|p| p.join("models"))
    }

    /// Load configuration from the default path.
    /// Returns defaults if the file doesn't exist.
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        Self::load_from(&path)
    }

    /// Load configuration from a specific path.
    /// Returns defaults if the file doesn't exist.
    pub fn load_from(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        Self::parse(&content)
    }

    /// Parse configuration from a TOML string.
    pub fn parse(content: &str) -> Result<Self> {
        toml::from_str(content).context("Failed to parse config file as TOML")
    }

    /// Save configuration to the default path.
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        self.save_to(&path)
    }

    /// Save configuration to a specific path.
    pub fn save_to(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory: {}", parent.display())
            })?;
        }

        let content = toml::to_string_pretty(self).context("Failed to serialize config to TOML")?;

        std::fs::write(path, content)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;

        Ok(())
    }
}

#[cfg(test)]
#[path = "config_test.rs"]
mod tests;
