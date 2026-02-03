//! Speech-to-text transcription.
//!
//! This module provides a trait abstraction for transcription backends
//! and implementations for specific models.

use anyhow::Result;

mod whisper;

pub use whisper::WhisperTranscriber;

/// Speech-to-text transcriber.
///
/// Implementations convert audio samples to text.
pub trait Transcriber: Send {
    /// Transcribe audio samples to text.
    ///
    /// # Arguments
    /// * `audio` - Audio samples as f32, expected to be 16kHz mono
    /// * `sample_rate` - Sample rate of the audio in Hz (must be 16000)
    ///
    /// # Returns
    /// The transcribed text, or an error if transcription failed.
    fn transcribe(&mut self, audio: &[f32], sample_rate: u32) -> Result<String>;
}
