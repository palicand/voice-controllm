//! NVIDIA Canary (NeMo) transcription backend.
//!
//! Uses the Canary 1B model via ONNX Runtime for speech-to-text.

use super::Transcriber;
use anyhow::Result;
use std::path::Path;
use tracing::debug;

/// Canary speech-to-text transcriber.
///
/// Uses NVIDIA's Canary 1B model for multilingual transcription.
pub struct CanaryTranscriber {
    // TODO: Add ONNX session once we have the model format figured out
    languages: Vec<String>,
}

impl CanaryTranscriber {
    /// Create a new Canary transcriber.
    ///
    /// # Arguments
    /// * `model_path` - Path to the Canary ONNX model
    /// * `languages` - Languages to recognize (e.g., ["en", "de", "cs"])
    pub fn new(model_path: impl AsRef<Path>, languages: Vec<String>) -> Result<Self> {
        debug!(
            path = %model_path.as_ref().display(),
            languages = ?languages,
            "Loading Canary model"
        );

        // TODO: Load ONNX model
        // For now, just validate the path exists
        if !model_path.as_ref().exists() {
            anyhow::bail!(
                "Canary model not found at {}",
                model_path.as_ref().display()
            );
        }

        Ok(Self { languages })
    }

    /// Get the configured languages.
    pub fn languages(&self) -> &[String] {
        &self.languages
    }
}

impl Transcriber for CanaryTranscriber {
    fn transcribe(&self, audio: &[f32], sample_rate: u32) -> Result<String> {
        debug!(
            samples = audio.len(),
            sample_rate = sample_rate,
            duration_secs = audio.len() as f32 / sample_rate as f32,
            "Transcribing audio"
        );

        // TODO: Implement actual transcription
        // For now, return a placeholder
        Ok(String::from("[transcription not yet implemented]"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_languages() {
        // Can't test new() without a model file, but we can test the struct
        let transcriber = CanaryTranscriber {
            languages: vec!["en".to_string(), "de".to_string()],
        };
        assert_eq!(transcriber.languages(), &["en", "de"]);
    }
}
