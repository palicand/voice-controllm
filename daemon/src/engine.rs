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
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Events emitted during engine initialization.
#[derive(Debug, Clone)]
pub enum InitEvent {
    /// Model is being downloaded.
    Downloading {
        model: String,
        bytes: u64,
        total: u64,
    },
    /// Model is being loaded into memory.
    Loading { model: String },
    /// Engine is ready.
    Ready,
}

/// Loaded model components ready for audio processing.
struct InitializedComponents {
    vad: VoiceActivityDetector,
    transcriber: WhisperTranscriber,
}

/// Transcription engine.
pub struct Engine {
    config: Config,
    model_manager: ModelManager,
    components: Option<InitializedComponents>,
}

impl Engine {
    /// Create a new engine with the given configuration.
    pub fn new(config: Config) -> Result<Self> {
        let model_manager = ModelManager::new()?;
        Ok(Self {
            config,
            model_manager,
            components: None,
        })
    }

    /// Create a new engine with a custom model manager.
    pub fn with_model_manager(config: Config, model_manager: ModelManager) -> Self {
        Self {
            config,
            model_manager,
            components: None,
        }
    }

    /// Check if the engine has been initialized (models loaded).
    pub fn is_initialized(&self) -> bool {
        self.components.is_some()
    }

    /// Initialize the engine: download and load models.
    ///
    /// Calls `on_progress` with status updates suitable for UI display.
    /// After this returns Ok(()), the engine is ready for `run_loop()`.
    pub async fn initialize(&mut self, on_progress: impl Fn(InitEvent) + Send) -> Result<()> {
        info!("Initializing engine");

        // Ensure VAD model
        on_progress(InitEvent::Loading {
            model: "silero-vad".to_string(),
        });
        let vad_model_path = self
            .model_manager
            .ensure_model(ModelId::SileroVad)
            .await
            .context("Failed to ensure VAD model")?;

        // Ensure Whisper model
        let whisper_model_id = speech_model_to_model_id(self.config.model.model);
        on_progress(InitEvent::Loading {
            model: whisper_model_id.to_string(),
        });
        let whisper_model_path = self
            .model_manager
            .ensure_model(whisper_model_id)
            .await
            .context("Failed to ensure Whisper model")?;

        info!("Models ready, initializing components");

        // Initialize VAD
        let vad = VoiceActivityDetector::new(&vad_model_path, VadConfig::default())
            .context("Failed to initialize VAD")?;

        // Initialize transcriber
        let language = if self.config.model.language == "auto" {
            None
        } else {
            Some(self.config.model.language.clone())
        };
        let transcriber = WhisperTranscriber::new(&whisper_model_path, language)
            .context("Failed to initialize Whisper")?;

        self.components = Some(InitializedComponents { vad, transcriber });

        on_progress(InitEvent::Ready);
        info!("Engine initialized");

        Ok(())
    }

    /// Run the audio capture and transcription loop.
    ///
    /// Blocks until the `cancel` token is cancelled.
    /// Requires `initialize()` to have been called first.
    pub async fn run_loop(
        &mut self,
        cancel: CancellationToken,
        mut on_transcription: impl FnMut(&str),
    ) -> Result<()> {
        let components = self
            .components
            .as_mut()
            .context("Engine not initialized — call initialize() first")?;

        info!("Starting audio capture");

        let capture = AudioCapture::start().context("Failed to start audio capture")?;
        let sample_rate = capture.sample_rate();
        info!(
            sample_rate = sample_rate,
            target_rate = TARGET_SAMPLE_RATE,
            "Audio capture started"
        );

        let mut resampler = AudioResampler::new(sample_rate, TARGET_SAMPLE_RATE, 1024)
            .context("Failed to create resampler")?;

        let mut audio = AudioBuffers {
            input: Vec::new(),
            vad: Vec::new(),
            speech: Vec::new(),
            resampler_chunk: resampler.chunk_size(),
            vad_chunk: components.vad.chunk_size(),
        };

        info!("Listening for speech...");

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    info!("Cancellation received, stopping audio capture");
                    break;
                }
                _ = tokio::time::sleep(std::time::Duration::from_millis(10)) => {
                    if let Some(samples) = capture.try_recv() {
                        audio.input.extend(samples);
                        resample_input(&mut audio, &mut resampler);
                        process_vad_chunks(components, &mut audio, &mut on_transcription);
                    }
                }
            }
        }

        capture.stop();
        info!("Audio capture stopped");

        Ok(())
    }

    /// Run the full pipeline (initialize + loop). Convenience for examples/tests.
    #[deprecated(note = "prefer calling initialize() + run_loop() separately")]
    pub async fn run<F>(&mut self, running: Arc<AtomicBool>, on_transcription: F) -> Result<()>
    where
        F: FnMut(&str),
    {
        self.initialize(|_| {}).await?;

        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();

        // Bridge AtomicBool to CancellationToken
        tokio::spawn(async move {
            loop {
                if !running.load(Ordering::SeqCst) {
                    cancel_clone.cancel();
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        });

        self.run_loop(cancel, on_transcription).await
    }
}

/// Buffers used during the audio processing loop.
struct AudioBuffers {
    input: Vec<f32>,
    vad: Vec<f32>,
    speech: Vec<f32>,
    resampler_chunk: usize,
    vad_chunk: usize,
}

/// Drain complete chunks from the input buffer and resample into the VAD buffer.
fn resample_input(audio: &mut AudioBuffers, resampler: &mut AudioResampler) {
    while audio.input.len() >= audio.resampler_chunk {
        let chunk: Vec<f32> = audio.input.drain(..audio.resampler_chunk).collect();
        if let Ok(resampled) = resampler.process(&chunk) {
            audio.vad.extend(resampled);
        }
    }
}

/// Process complete VAD-sized chunks, detecting speech boundaries and transcribing.
fn process_vad_chunks(
    components: &mut InitializedComponents,
    audio: &mut AudioBuffers,
    on_transcription: &mut impl FnMut(&str),
) {
    while audio.vad.len() >= audio.vad_chunk {
        let chunk: Vec<f32> = audio.vad.drain(..audio.vad_chunk).collect();

        if components.vad.is_speaking() {
            audio.speech.extend(&chunk);
        }

        match components.vad.process(&chunk) {
            Ok(Some(VadEvent::SpeechStart)) => {
                debug!("Speech started");
                audio.speech.clear();
                audio.speech.extend(&chunk);
            }
            Ok(Some(VadEvent::SpeechEnd)) => {
                transcribe_speech(components, audio, on_transcription);
            }
            Ok(None) => {}
            Err(e) => {
                warn!(error = %e, "VAD processing error");
            }
        }
    }
}

/// Transcribe accumulated speech and clear the buffer.
fn transcribe_speech(
    components: &mut InitializedComponents,
    audio: &mut AudioBuffers,
    on_transcription: &mut impl FnMut(&str),
) {
    debug!(
        samples = audio.speech.len(),
        duration_secs = audio.speech.len() as f32 / VAD_SAMPLE_RATE as f32,
        "Speech ended, transcribing"
    );

    if !audio.speech.is_empty() {
        match components
            .transcriber
            .transcribe(&audio.speech, VAD_SAMPLE_RATE)
        {
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
    audio.speech.clear();
}

/// Convert SpeechModel config to ModelId for download.
pub(crate) fn speech_model_to_model_id(model: SpeechModel) -> ModelId {
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
