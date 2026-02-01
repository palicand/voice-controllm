//! Voice Activity Detection using Silero VAD.
//!
//! Detects speech segments in audio using the Silero VAD ONNX model.

use anyhow::{Context, Result};
use ndarray::{Array0, Array2, Array3};
use ort::session::Session;
use ort::value::TensorRef;
use std::path::Path;
use tracing::{debug, trace};

/// LSTM hidden state size for Silero VAD.
const LSTM_HIDDEN_SIZE: usize = 128;

/// Context size for 16kHz audio (prepended to each chunk).
const CONTEXT_SIZE_16K: usize = 64;

/// Sample rate expected by Silero VAD.
pub const VAD_SAMPLE_RATE: u32 = 16000;

/// Supported chunk sizes for Silero VAD (in samples at 16kHz).
pub const VAD_CHUNK_SIZES: [usize; 3] = [512, 1024, 1536];

/// Default speech probability threshold.
pub const DEFAULT_THRESHOLD: f32 = 0.5;

/// VAD event indicating speech state changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VadEvent {
    /// Speech started.
    SpeechStart,
    /// Speech ended.
    SpeechEnd,
}

/// Configuration for the VAD state machine.
#[derive(Debug, Clone)]
pub struct VadConfig {
    /// Probability threshold for considering audio as speech.
    pub threshold: f32,
    /// Minimum consecutive speech chunks before triggering SpeechStart.
    pub min_speech_chunks: usize,
    /// Minimum consecutive silence chunks before triggering SpeechEnd.
    pub min_silence_chunks: usize,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            threshold: DEFAULT_THRESHOLD,
            min_speech_chunks: 2,
            min_silence_chunks: 8,
        }
    }
}

/// State machine for tracking speech/silence transitions.
#[derive(Debug)]
pub struct VadStateMachine {
    config: VadConfig,
    is_speaking: bool,
    speech_chunk_count: usize,
    silence_chunk_count: usize,
}

impl VadStateMachine {
    /// Create a new VAD state machine.
    pub fn new(config: VadConfig) -> Self {
        Self {
            config,
            is_speaking: false,
            speech_chunk_count: 0,
            silence_chunk_count: 0,
        }
    }

    /// Process a speech probability and return any state change event.
    pub fn process(&mut self, probability: f32) -> Option<VadEvent> {
        let is_speech = probability >= self.config.threshold;

        trace!(
            probability = probability,
            threshold = self.config.threshold,
            is_speech = is_speech,
            speaking = self.is_speaking,
            speech_chunks = self.speech_chunk_count,
            silence_chunks = self.silence_chunk_count,
            "VAD state machine processing"
        );

        if is_speech {
            self.speech_chunk_count += 1;
            self.silence_chunk_count = 0;

            if !self.is_speaking && self.speech_chunk_count >= self.config.min_speech_chunks {
                self.is_speaking = true;
                debug!("Speech started");
                return Some(VadEvent::SpeechStart);
            }
        } else {
            self.silence_chunk_count += 1;
            self.speech_chunk_count = 0;

            if self.is_speaking && self.silence_chunk_count >= self.config.min_silence_chunks {
                self.is_speaking = false;
                debug!("Speech ended");
                return Some(VadEvent::SpeechEnd);
            }
        }

        None
    }

    /// Check if currently in speaking state.
    pub fn is_speaking(&self) -> bool {
        self.is_speaking
    }

    /// Reset the state machine.
    pub fn reset(&mut self) {
        self.is_speaking = false;
        self.speech_chunk_count = 0;
        self.silence_chunk_count = 0;
    }
}

/// Voice Activity Detector using Silero VAD ONNX model.
pub struct VoiceActivityDetector {
    session: Session,
    /// LSTM state: shape (2, 1, 128) - combines h and c states.
    state: Array3<f32>,
    /// Audio context from previous chunk (64 samples at 16kHz).
    context: Vec<f32>,
    state_machine: VadStateMachine,
    chunk_size: usize,
}

impl VoiceActivityDetector {
    /// Load the Silero VAD model from the given path.
    pub fn new(model_path: impl AsRef<Path>, config: VadConfig) -> Result<Self> {
        Self::with_chunk_size(model_path, config, 512)
    }

    /// Load the model with a specific chunk size.
    pub fn with_chunk_size(
        model_path: impl AsRef<Path>,
        config: VadConfig,
        chunk_size: usize,
    ) -> Result<Self> {
        if !VAD_CHUNK_SIZES.contains(&chunk_size) {
            anyhow::bail!(
                "Invalid chunk size {}. Must be one of {:?}",
                chunk_size,
                VAD_CHUNK_SIZES
            );
        }

        debug!(
            path = %model_path.as_ref().display(),
            chunk_size = chunk_size,
            "Loading VAD model"
        );

        let session = Session::builder()
            .context("Failed to create ONNX session builder")?
            .with_intra_threads(1)
            .context("Failed to set intra threads")?
            .commit_from_file(model_path.as_ref())
            .with_context(|| {
                format!(
                    "Failed to load VAD model from {}",
                    model_path.as_ref().display()
                )
            })?;

        debug!("VAD model loaded successfully");

        // Initialize LSTM state: (2, batch=1, hidden_size=128)
        let state = Array3::<f32>::zeros((2, 1, LSTM_HIDDEN_SIZE));
        // Initialize context buffer with zeros
        let context = vec![0.0f32; CONTEXT_SIZE_16K];

        Ok(Self {
            session,
            state,
            context,
            state_machine: VadStateMachine::new(config),
            chunk_size,
        })
    }

    /// Process an audio chunk and return the speech probability.
    /// Audio must be f32 samples at 16kHz, mono.
    pub fn process_chunk(&mut self, audio: &[f32]) -> Result<f32> {
        if audio.len() != self.chunk_size {
            anyhow::bail!(
                "Audio chunk size {} doesn't match expected {}",
                audio.len(),
                self.chunk_size
            );
        }

        // Prepend context to audio input (required by Silero VAD)
        let mut input_with_context = self.context.clone();
        input_with_context.extend_from_slice(audio);

        // Prepare input tensors
        let audio_array =
            Array2::from_shape_vec((1, self.chunk_size + CONTEXT_SIZE_16K), input_with_context)
                .context("Failed to create audio array")?;
        let sr_array = Array0::from_elem((), VAD_SAMPLE_RATE as i64);

        // Run inference
        let input_tensor = TensorRef::from_array_view(&audio_array)?;
        let sr_tensor = TensorRef::from_array_view(&sr_array)?;
        let state_tensor = TensorRef::from_array_view(&self.state)?;

        let outputs = self
            .session
            .run(ort::inputs![
                "input" => input_tensor,
                "sr" => sr_tensor,
                "state" => state_tensor
            ])
            .context("VAD inference failed")?;

        // Update context with last 64 samples for next chunk
        self.context = audio[audio.len() - CONTEXT_SIZE_16K..].to_vec();

        // Extract output probability
        let (_, output_data) = outputs["output"]
            .try_extract_tensor::<f32>()
            .context("Failed to extract output tensor")?;
        let probability = output_data.first().copied().unwrap_or(0.0);

        // Audio level stats for debugging
        let rms = (audio.iter().map(|x| x * x).sum::<f32>() / audio.len() as f32).sqrt();
        let max_abs = audio.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
        trace!(
            probability = probability,
            rms = rms,
            max_abs = max_abs,
            "VAD inference complete"
        );

        // Update state for next call
        let (_, state_data) = outputs["stateN"]
            .try_extract_tensor::<f32>()
            .context("Failed to extract state tensor")?;

        self.state = Array3::from_shape_vec((2, 1, LSTM_HIDDEN_SIZE), state_data.to_vec())
            .context("Failed to reshape state")?;

        Ok(probability)
    }

    /// Process audio and return any VAD event.
    pub fn process(&mut self, audio: &[f32]) -> Result<Option<VadEvent>> {
        let probability = self.process_chunk(audio)?;
        Ok(self.state_machine.process(probability))
    }

    /// Check if currently detecting speech.
    pub fn is_speaking(&self) -> bool {
        self.state_machine.is_speaking()
    }

    /// Reset the detector state.
    pub fn reset(&mut self) {
        self.state = Array3::<f32>::zeros((2, 1, LSTM_HIDDEN_SIZE));
        self.context = vec![0.0f32; CONTEXT_SIZE_16K];
        self.state_machine.reset();
    }

    /// Get the expected chunk size.
    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }
}

#[cfg(test)]
#[path = "vad_test.rs"]
mod tests;
