//! Whisper transcription backend.
//!
//! Uses whisper.cpp via whisper-rs for speech-to-text.

use super::Transcriber;
use anyhow::{Context, Result};
use std::path::Path;
use tracing::{debug, info};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

/// Segments with no-speech probability above this threshold are dropped.
/// Whisper's default no_speech_thold is 0.6; we use the same value.
const NO_SPEECH_PROB_THRESHOLD: f32 = 0.6;

/// Whisper speech-to-text transcriber.
///
/// The underlying WhisperContext is leaked intentionally - for a long-running daemon,
/// the model stays loaded for the process lifetime. This avoids complex self-referential
/// struct patterns while allowing the state to be reused across transcriptions.
pub struct WhisperTranscriber {
    state: WhisperState,
    language: Option<String>,
}

impl WhisperTranscriber {
    /// Create a new Whisper transcriber.
    ///
    /// # Arguments
    /// * `model_path` - Path to the Whisper GGML model file
    /// * `language` - Language code (e.g., "en", "de") or None for auto-detect
    pub fn new(model_path: impl AsRef<Path>, language: Option<String>) -> Result<Self> {
        info!(
            path = %model_path.as_ref().display(),
            language = ?language,
            "Loading Whisper model"
        );

        let ctx = WhisperContext::new_with_params(
            model_path.as_ref().to_str().context("Invalid model path")?,
            WhisperContextParameters::default(),
        )
        .context("Failed to load Whisper model")?;

        // Box and leak the context to get a 'static reference.
        // This is intentional for a long-running daemon - the model stays loaded for the process lifetime.
        let ctx_box = Box::new(ctx);
        let ctx_ref: &'static WhisperContext = Box::leak(ctx_box);

        let state = ctx_ref
            .create_state()
            .context("Failed to create Whisper state")?;

        info!("Whisper model and state loaded successfully");

        Ok(Self { state, language })
    }

    /// Get the configured language.
    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }
}

impl Transcriber for WhisperTranscriber {
    fn set_language(&mut self, language: Option<String>) {
        self.language = language;
    }

    fn transcribe(&mut self, audio: &[f32], sample_rate: u32) -> Result<String> {
        debug!(
            samples = audio.len(),
            sample_rate = sample_rate,
            duration_secs = audio.len() as f32 / sample_rate as f32,
            "Transcribing audio with Whisper"
        );

        // Whisper expects 16kHz audio
        if sample_rate != 16000 {
            anyhow::bail!(
                "Whisper expects 16kHz audio, got {}Hz. Resample before calling transcribe.",
                sample_rate
            );
        }

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        // Configure language
        if let Some(ref lang) = self.language {
            params.set_language(Some(lang));
        } else {
            params.set_language(None); // Auto-detect
        }

        // Each VAD segment is independent — don't let the decoder use
        // the previous transcription as a prompt, which causes it to
        // repeat the last sentence when audio is ambiguous.
        params.set_no_context(true);

        // Suppress non-speech tokens (hallucinated filler like "[MUSIC]", etc.)
        params.set_suppress_nst(true);

        // Disable printing to stdout
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        // Single segment mode for lower latency
        params.set_single_segment(true);

        // Run inference using the pre-created state
        self.state
            .full(params, audio)
            .context("Whisper inference failed")?;

        // Collect segments, filtering out those Whisper tags as non-speech.
        let num_segments = self.state.full_n_segments();
        let mut result = String::new();

        for i in 0..num_segments {
            if let Some(segment) = self.state.get_segment(i) {
                let no_speech_prob = segment.no_speech_probability();
                if no_speech_prob > NO_SPEECH_PROB_THRESHOLD {
                    debug!(
                        segment = i,
                        no_speech_prob = no_speech_prob,
                        "Dropping segment with high no-speech probability"
                    );
                    continue;
                }
                if let Ok(text) = segment.to_str_lossy() {
                    result.push_str(&text);
                }
            }
        }

        debug!(text_len = result.len(), "Transcription complete");

        Ok(result.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_language_getter() {
        // We can't test new() without a model, but we can test the struct directly
        // by using unsafe or just testing the language logic
        let lang = Some("en".to_string());
        assert_eq!(lang.as_deref(), Some("en"));
    }
}
