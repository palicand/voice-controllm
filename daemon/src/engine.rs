//! Transcription engine that coordinates the audio pipeline.
//!
//! The engine owns and orchestrates:
//! - Audio capture from microphone
//! - Resampling to 16kHz
//! - Voice activity detection
//! - Speech-to-text transcription

use crate::audio::{AudioCapture, AudioResampler, TARGET_SAMPLE_RATE};
use crate::config::{Config, SpeechModel};
use crate::models::{ModelId, ModelManager};
use crate::transcribe::{Transcriber, WhisperTranscriber};
use crate::vad::{VAD_SAMPLE_RATE, VadConfig, VadEvent, VoiceActivityDetector};
use anyhow::{Context, Result};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{debug, error, info, warn};

/// Engine state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineState {
    /// Engine is stopped.
    Stopped,
    /// Engine is running and listening.
    Listening,
    /// Engine is paused (not processing audio).
    Paused,
}

/// Transcription engine.
pub struct Engine {
    config: Config,
    model_manager: ModelManager,
    state: EngineState,
}

impl Engine {
    /// Create a new engine with the given configuration.
    pub fn new(config: Config) -> Result<Self> {
        let model_manager = ModelManager::new()?;
        Ok(Self {
            config,
            model_manager,
            state: EngineState::Stopped,
        })
    }

    /// Create a new engine with a custom model manager.
    pub fn with_model_manager(config: Config, model_manager: ModelManager) -> Self {
        Self {
            config,
            model_manager,
            state: EngineState::Stopped,
        }
    }

    /// Get the current engine state.
    pub fn state(&self) -> EngineState {
        self.state
    }

    /// Run the transcription engine.
    ///
    /// This method blocks and continuously processes audio until the
    /// `running` flag is set to false.
    ///
    /// # Arguments
    /// * `running` - Atomic flag to control the engine loop
    /// * `on_transcription` - Callback invoked with transcribed text
    pub async fn run<F>(&mut self, running: Arc<AtomicBool>, mut on_transcription: F) -> Result<()>
    where
        F: FnMut(&str),
    {
        info!("Starting transcription engine");
        self.state = EngineState::Listening;

        // Ensure models are downloaded
        let vad_model_path = self
            .model_manager
            .ensure_model(ModelId::SileroVad)
            .await
            .context("Failed to ensure VAD model")?;

        let whisper_model_id = speech_model_to_model_id(self.config.model.model);
        let whisper_model_path = self
            .model_manager
            .ensure_model(whisper_model_id)
            .await
            .context("Failed to ensure Whisper model")?;

        info!("Models ready, initializing components");

        // Initialize VAD
        let mut vad = VoiceActivityDetector::new(&vad_model_path, VadConfig::default())
            .context("Failed to initialize VAD")?;

        // Initialize transcriber
        let language = if self.config.model.languages.first().map(|s| s.as_str()) == Some("auto") {
            None
        } else {
            self.config.model.languages.first().cloned()
        };
        let mut transcriber = WhisperTranscriber::new(&whisper_model_path, language)
            .context("Failed to initialize Whisper")?;

        // Initialize audio capture
        let capture = AudioCapture::start().context("Failed to start audio capture")?;
        let sample_rate = capture.sample_rate();
        info!(
            sample_rate = sample_rate,
            target_rate = TARGET_SAMPLE_RATE,
            "Audio capture started"
        );

        // Initialize resampler
        let mut resampler = AudioResampler::new(sample_rate, TARGET_SAMPLE_RATE, 1024)
            .context("Failed to create resampler")?;

        // Buffers
        let mut input_buffer: Vec<f32> = Vec::new();
        let mut vad_buffer: Vec<f32> = Vec::new();
        let mut speech_buffer: Vec<f32> = Vec::new();

        let resampler_chunk = resampler.chunk_size();
        let vad_chunk_size = vad.chunk_size();

        info!("Engine running, listening for speech...");

        while running.load(Ordering::SeqCst) {
            // Receive audio samples
            if let Some(samples) = capture.try_recv() {
                input_buffer.extend(samples);

                // Process complete resampler chunks
                while input_buffer.len() >= resampler_chunk {
                    let chunk: Vec<f32> = input_buffer.drain(..resampler_chunk).collect();
                    if let Ok(resampled) = resampler.process(&chunk) {
                        vad_buffer.extend(resampled);
                    }
                }

                // Process complete VAD chunks
                while vad_buffer.len() >= vad_chunk_size {
                    let chunk: Vec<f32> = vad_buffer.drain(..vad_chunk_size).collect();

                    // Always buffer audio when VAD is speaking
                    if vad.is_speaking() {
                        speech_buffer.extend(&chunk);
                    }

                    // Process VAD
                    match vad.process(&chunk) {
                        Ok(Some(VadEvent::SpeechStart)) => {
                            debug!("Speech started");
                            // Start buffering (include this chunk)
                            speech_buffer.clear();
                            speech_buffer.extend(&chunk);
                        }
                        Ok(Some(VadEvent::SpeechEnd)) => {
                            debug!(
                                samples = speech_buffer.len(),
                                duration_secs = speech_buffer.len() as f32 / VAD_SAMPLE_RATE as f32,
                                "Speech ended, transcribing"
                            );

                            // Transcribe the buffered speech
                            if !speech_buffer.is_empty() {
                                match transcriber.transcribe(&speech_buffer, VAD_SAMPLE_RATE) {
                                    Ok(text) => {
                                        if !text.is_empty() {
                                            info!(text = %text, "Transcription complete");
                                            on_transcription(&text);
                                        }
                                    }
                                    Err(e) => {
                                        error!(error = %e, "Transcription failed");
                                    }
                                }
                            }
                            speech_buffer.clear();
                        }
                        Ok(None) => {}
                        Err(e) => {
                            warn!(error = %e, "VAD processing error");
                        }
                    }
                }
            }

            // Small sleep to avoid busy-waiting
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        capture.stop();
        self.state = EngineState::Stopped;
        info!("Engine stopped");

        Ok(())
    }
}

/// Convert SpeechModel config to ModelId for download.
fn speech_model_to_model_id(model: SpeechModel) -> ModelId {
    match model {
        SpeechModel::WhisperTiny => ModelId::WhisperTiny,
        SpeechModel::WhisperTinyEn => ModelId::WhisperTinyEn,
        SpeechModel::WhisperBase => ModelId::WhisperBase,
        SpeechModel::WhisperBaseEn => ModelId::WhisperBaseEn,
        SpeechModel::WhisperSmall => ModelId::WhisperSmall,
        SpeechModel::WhisperSmallEn => ModelId::WhisperSmallEn,
        SpeechModel::WhisperMedium => ModelId::WhisperMedium,
        SpeechModel::WhisperMediumEn => ModelId::WhisperMediumEn,
        SpeechModel::WhisperLargeV3 => ModelId::WhisperLargeV3,
        SpeechModel::WhisperLargeV3Turbo => ModelId::WhisperLargeV3Turbo,
    }
}

#[cfg(test)]
#[path = "engine_test.rs"]
mod tests;
