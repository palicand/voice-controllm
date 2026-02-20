//! Audio capture and processing for VCM daemon.
//!
//! Handles microphone input capture and resampling to 16kHz mono for speech recognition.

use anyhow::{Context, Result};
use audioadapter_buffers::direct::SequentialSliceOfVecs;
use rubato::audioadapter::Adapter;
use rubato::{Fft, FixedSync, Resampler};
use std::sync::mpsc;

/// Target sample rate for speech recognition models.
pub const TARGET_SAMPLE_RATE: u32 = 16000;

/// Audio buffer containing mono f32 samples at a known sample rate.
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

impl AudioBuffer {
    /// Create a new audio buffer.
    pub fn new(samples: Vec<f32>, sample_rate: u32) -> Self {
        Self {
            samples,
            sample_rate,
        }
    }

    /// Create an empty buffer at the given sample rate.
    pub fn empty(sample_rate: u32) -> Self {
        Self {
            samples: Vec::new(),
            sample_rate,
        }
    }

    /// Duration of the buffer in seconds.
    pub fn duration_secs(&self) -> f32 {
        if self.sample_rate == 0 {
            return 0.0;
        }
        self.samples.len() as f32 / self.sample_rate as f32
    }

    /// Append samples from another buffer. Panics if sample rates don't match.
    pub fn append(&mut self, other: &AudioBuffer) {
        assert_eq!(
            self.sample_rate, other.sample_rate,
            "Cannot append buffers with different sample rates"
        );
        self.samples.extend_from_slice(&other.samples);
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.samples.clear();
    }
}

/// Convert stereo interleaved samples to mono by averaging channels.
pub fn stereo_to_mono(stereo: &[f32]) -> Vec<f32> {
    stereo
        .chunks_exact(2)
        .map(|pair| (pair[0] + pair[1]) / 2.0)
        .collect()
}

/// Convert multi-channel interleaved samples to mono by averaging all channels.
pub fn to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    if channels == 1 {
        return samples.to_vec();
    }

    let channels = channels as usize;
    samples
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// Resampler for converting audio between sample rates.
pub struct AudioResampler {
    resampler: Fft<f32>,
    chunk_size_in: usize,
    chunk_size_out: usize,
}

impl AudioResampler {
    /// Create a new resampler.
    ///
    /// # Arguments
    /// * `input_rate` - Input sample rate in Hz
    /// * `output_rate` - Output sample rate in Hz
    /// * `chunk_size` - Number of input samples per processing chunk
    pub fn new(input_rate: u32, output_rate: u32, chunk_size: usize) -> Result<Self> {
        let resampler = Fft::new(
            input_rate as usize,
            output_rate as usize,
            chunk_size,
            1, // sub_chunks
            1, // channels
            FixedSync::Input,
        )
        .context("Failed to create resampler")?;

        let chunk_size_out = resampler.output_frames_max();

        Ok(Self {
            resampler,
            chunk_size_in: chunk_size,
            chunk_size_out,
        })
    }

    /// Resample audio data. Input length must be a multiple of chunk_size.
    pub fn process(&mut self, input: &[f32]) -> Result<Vec<f32>> {
        if input.is_empty() {
            return Ok(Vec::new());
        }

        let mut output = Vec::new();
        let input_chunks = input.chunks_exact(self.chunk_size_in);

        for chunk in input_chunks {
            let input_vecs = vec![chunk.to_vec()];
            let input_adapter =
                SequentialSliceOfVecs::new(&input_vecs, 1, chunk.len()).expect("valid input");
            let resampled = self
                .resampler
                .process(&input_adapter, 0, None)
                .context("Resampling failed")?;

            // Extract samples from the InterleavedOwned buffer
            for frame_idx in 0..resampled.frames() {
                output.push(resampled.read_sample(0, frame_idx).unwrap_or(0.0));
            }
        }

        Ok(output)
    }

    /// Get the required input chunk size.
    pub fn chunk_size(&self) -> usize {
        self.chunk_size_in
    }

    /// Get the output chunk size for a given input chunk.
    pub fn output_chunk_size(&self) -> usize {
        self.chunk_size_out
    }
}

/// Audio capture from the default input device.
pub struct AudioCapture {
    stream: cpal::Stream,
    receiver: mpsc::Receiver<Vec<f32>>,
    sample_rate: u32,
    channels: u16,
}

impl AudioCapture {
    /// Start capturing audio from the default input device.
    pub fn start() -> Result<Self> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .context("No input device available")?;

        let config = device
            .default_input_config()
            .context("Failed to get default input config")?;

        let sample_rate = config.sample_rate();
        let channels = config.channels();

        let (sender, receiver) = mpsc::channel();

        let err_fn = |err| eprintln!("Audio stream error: {}", err);

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream(
                &config.into(),
                move |data: &[f32], _| {
                    let _ = sender.send(data.to_vec());
                },
                err_fn,
                None,
            ),
            cpal::SampleFormat::I16 => device.build_input_stream(
                &config.into(),
                move |data: &[i16], _| {
                    let samples: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
                    let _ = sender.send(samples);
                },
                err_fn,
                None,
            ),
            cpal::SampleFormat::U16 => device.build_input_stream(
                &config.into(),
                move |data: &[u16], _| {
                    let samples: Vec<f32> = data
                        .iter()
                        .map(|&s| (s as f32 - 32768.0) / 32768.0)
                        .collect();
                    let _ = sender.send(samples);
                },
                err_fn,
                None,
            ),
            format => anyhow::bail!("Unsupported sample format: {:?}", format),
        }
        .context("Failed to build input stream")?;

        stream.play().context("Failed to start audio stream")?;

        Ok(Self {
            stream,
            receiver,
            sample_rate,
            channels,
        })
    }

    /// Get the native sample rate of the input device.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get the number of channels.
    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// Try to receive available audio samples (non-blocking).
    /// Returns mono samples at the device's native sample rate.
    pub fn try_recv(&self) -> Option<Vec<f32>> {
        let mut all_samples = Vec::new();

        // Drain all available samples
        while let Ok(samples) = self.receiver.try_recv() {
            all_samples.extend(samples);
        }

        if all_samples.is_empty() {
            return None;
        }

        // Convert to mono
        Some(to_mono(&all_samples, self.channels))
    }

    /// Stop the audio stream.
    pub fn stop(self) {
        use cpal::traits::StreamTrait;
        let _ = self.stream.pause();
        drop(self);
    }
}

#[cfg(test)]
#[path = "audio_test.rs"]
mod tests;
