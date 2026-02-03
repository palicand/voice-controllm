//! Model download and management.
//!
//! Handles automatic downloading of ML models on first run.

use anyhow::{Context, Result};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::{debug, info, warn};

/// Identifier for downloadable models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelId {
    /// Silero VAD model for voice activity detection.
    SileroVad,
    /// Whisper tiny model (~75MB).
    WhisperTiny,
    /// Whisper tiny English-only model (~75MB).
    WhisperTinyEn,
    /// Whisper base model (~150MB).
    WhisperBase,
    /// Whisper base English-only model (~150MB).
    WhisperBaseEn,
    /// Whisper small model (~500MB).
    WhisperSmall,
    /// Whisper small English-only model (~500MB).
    WhisperSmallEn,
    /// Whisper medium model (~1.5GB).
    WhisperMedium,
    /// Whisper medium English-only model (~1.5GB).
    WhisperMediumEn,
    /// Whisper large-v3 model (~3GB).
    WhisperLargeV3,
    /// Whisper large-v3-turbo model (~1.5GB).
    WhisperLargeV3Turbo,
}

const WHISPER_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

impl ModelId {
    /// Get model metadata.
    fn info(&self) -> ModelInfo {
        match self {
            ModelId::SileroVad => ModelInfo {
                filename: "silero_vad.onnx",
                url: "https://github.com/snakers4/silero-vad/raw/master/src/silero_vad/data/silero_vad.onnx".to_string(),
                size_bytes: Some(2_327_524),
                coreml_encoder: None,
            },
            ModelId::WhisperTiny => ModelInfo {
                filename: "ggml-tiny.bin",
                url: format!("{}/ggml-tiny.bin", WHISPER_BASE_URL),
                size_bytes: Some(77_691_713),
                coreml_encoder: Some(CoreMlModelInfo {
                    zip_filename: "ggml-tiny-encoder.mlmodelc.zip",
                    extracted_dirname: "ggml-tiny-encoder.mlmodelc",
                    url: format!("{}/ggml-tiny-encoder.mlmodelc.zip", WHISPER_BASE_URL),
                }),
            },
            ModelId::WhisperTinyEn => ModelInfo {
                filename: "ggml-tiny.en.bin",
                url: format!("{}/ggml-tiny.en.bin", WHISPER_BASE_URL),
                size_bytes: Some(77_704_715),
                coreml_encoder: Some(CoreMlModelInfo {
                    zip_filename: "ggml-tiny.en-encoder.mlmodelc.zip",
                    extracted_dirname: "ggml-tiny.en-encoder.mlmodelc",
                    url: format!("{}/ggml-tiny.en-encoder.mlmodelc.zip", WHISPER_BASE_URL),
                }),
            },
            ModelId::WhisperBase => ModelInfo {
                filename: "ggml-base.bin",
                url: format!("{}/ggml-base.bin", WHISPER_BASE_URL),
                size_bytes: Some(147_951_465),
                coreml_encoder: Some(CoreMlModelInfo {
                    zip_filename: "ggml-base-encoder.mlmodelc.zip",
                    extracted_dirname: "ggml-base-encoder.mlmodelc",
                    url: format!("{}/ggml-base-encoder.mlmodelc.zip", WHISPER_BASE_URL),
                }),
            },
            ModelId::WhisperBaseEn => ModelInfo {
                filename: "ggml-base.en.bin",
                url: format!("{}/ggml-base.en.bin", WHISPER_BASE_URL),
                size_bytes: Some(147_964_211),
                coreml_encoder: Some(CoreMlModelInfo {
                    zip_filename: "ggml-base.en-encoder.mlmodelc.zip",
                    extracted_dirname: "ggml-base.en-encoder.mlmodelc",
                    url: format!("{}/ggml-base.en-encoder.mlmodelc.zip", WHISPER_BASE_URL),
                }),
            },
            ModelId::WhisperSmall => ModelInfo {
                filename: "ggml-small.bin",
                url: format!("{}/ggml-small.bin", WHISPER_BASE_URL),
                size_bytes: Some(487_601_967),
                coreml_encoder: Some(CoreMlModelInfo {
                    zip_filename: "ggml-small-encoder.mlmodelc.zip",
                    extracted_dirname: "ggml-small-encoder.mlmodelc",
                    url: format!("{}/ggml-small-encoder.mlmodelc.zip", WHISPER_BASE_URL),
                }),
            },
            ModelId::WhisperSmallEn => ModelInfo {
                filename: "ggml-small.en.bin",
                url: format!("{}/ggml-small.en.bin", WHISPER_BASE_URL),
                size_bytes: Some(487_614_201),
                coreml_encoder: Some(CoreMlModelInfo {
                    zip_filename: "ggml-small.en-encoder.mlmodelc.zip",
                    extracted_dirname: "ggml-small.en-encoder.mlmodelc",
                    url: format!("{}/ggml-small.en-encoder.mlmodelc.zip", WHISPER_BASE_URL),
                }),
            },
            ModelId::WhisperMedium => ModelInfo {
                filename: "ggml-medium.bin",
                url: format!("{}/ggml-medium.bin", WHISPER_BASE_URL),
                size_bytes: Some(1_533_774_781),
                coreml_encoder: Some(CoreMlModelInfo {
                    zip_filename: "ggml-medium-encoder.mlmodelc.zip",
                    extracted_dirname: "ggml-medium-encoder.mlmodelc",
                    url: format!("{}/ggml-medium-encoder.mlmodelc.zip", WHISPER_BASE_URL),
                }),
            },
            ModelId::WhisperMediumEn => ModelInfo {
                filename: "ggml-medium.en.bin",
                url: format!("{}/ggml-medium.en.bin", WHISPER_BASE_URL),
                size_bytes: Some(1_533_774_781),
                coreml_encoder: Some(CoreMlModelInfo {
                    zip_filename: "ggml-medium.en-encoder.mlmodelc.zip",
                    extracted_dirname: "ggml-medium.en-encoder.mlmodelc",
                    url: format!("{}/ggml-medium.en-encoder.mlmodelc.zip", WHISPER_BASE_URL),
                }),
            },
            ModelId::WhisperLargeV3 => ModelInfo {
                filename: "ggml-large-v3.bin",
                url: format!("{}/ggml-large-v3.bin", WHISPER_BASE_URL),
                size_bytes: Some(3_094_623_691),
                coreml_encoder: Some(CoreMlModelInfo {
                    zip_filename: "ggml-large-v3-encoder.mlmodelc.zip",
                    extracted_dirname: "ggml-large-v3-encoder.mlmodelc",
                    url: format!("{}/ggml-large-v3-encoder.mlmodelc.zip", WHISPER_BASE_URL),
                }),
            },
            ModelId::WhisperLargeV3Turbo => ModelInfo {
                filename: "ggml-large-v3-turbo.bin",
                url: format!("{}/ggml-large-v3-turbo.bin", WHISPER_BASE_URL),
                size_bytes: Some(1_624_555_275),
                coreml_encoder: Some(CoreMlModelInfo {
                    zip_filename: "ggml-large-v3-turbo-encoder.mlmodelc.zip",
                    extracted_dirname: "ggml-large-v3-turbo-encoder.mlmodelc",
                    url: format!("{}/ggml-large-v3-turbo-encoder.mlmodelc.zip", WHISPER_BASE_URL),
                }),
            },
        }
    }
}

/// Metadata for a downloadable model.
struct ModelInfo {
    /// Filename to save as.
    filename: &'static str,
    /// Download URL.
    url: String,
    /// Expected file size for validation (optional).
    size_bytes: Option<u64>,
    /// CoreML encoder model info (for Whisper models with CoreML support).
    coreml_encoder: Option<CoreMlModelInfo>,
}

/// Metadata for a CoreML model component.
struct CoreMlModelInfo {
    /// Zip filename to download.
    zip_filename: &'static str,
    /// Extracted directory name.
    extracted_dirname: &'static str,
    /// Download URL.
    url: String,
}

/// Manages model downloads and storage.
pub struct ModelManager {
    models_dir: PathBuf,
}

impl ModelManager {
    /// Create a new ModelManager using the default models directory.
    ///
    /// Default: `~/.local/share/voice-controllm/models/` (or `$XDG_DATA_HOME/voice-controllm/models/`)
    pub fn new() -> Result<Self> {
        let xdg = xdg::BaseDirectories::with_prefix("voice-controllm");
        let models_dir = xdg
            .get_data_home()
            .context("Could not determine data directory (HOME not set?)")?
            .join("models");
        Ok(Self { models_dir })
    }

    /// Create a ModelManager with a custom models directory.
    pub fn with_dir(models_dir: impl Into<PathBuf>) -> Self {
        Self {
            models_dir: models_dir.into(),
        }
    }

    /// Get the models directory path.
    pub fn models_dir(&self) -> &Path {
        &self.models_dir
    }

    /// Ensure a model is available, downloading if necessary.
    ///
    /// Returns the path to the model file.
    pub async fn ensure_model(&self, model: ModelId) -> Result<PathBuf> {
        let info = model.info();
        let model_path = self.models_dir.join(info.filename);

        let needs_download = if model_path.exists() {
            // Validate size if known
            if let Some(expected_size) = info.size_bytes {
                let metadata = fs::metadata(&model_path)
                    .await
                    .context("Failed to read model metadata")?;
                let actual_size = metadata.len();

                if actual_size != expected_size {
                    warn!(
                        model = ?model,
                        expected = expected_size,
                        actual = actual_size,
                        "Model size mismatch, re-downloading"
                    );
                    fs::remove_file(&model_path)
                        .await
                        .context("Failed to remove corrupted model")?;
                    true
                } else {
                    debug!(path = %model_path.display(), "Model already exists");
                    false
                }
            } else {
                debug!(path = %model_path.display(), "Model already exists");
                false
            }
        } else {
            true
        };

        if needs_download {
            // Download the model
            self.download_model(&info, &model_path).await?;
        }

        // Ensure CoreML encoder is available (macOS only)
        #[cfg(target_os = "macos")]
        if let Some(ref coreml) = info.coreml_encoder {
            self.ensure_coreml_encoder(coreml).await?;
        }

        Ok(model_path)
    }

    /// Ensure a CoreML encoder model is downloaded and extracted.
    #[cfg(target_os = "macos")]
    async fn ensure_coreml_encoder(&self, coreml: &CoreMlModelInfo) -> Result<()> {
        let extracted_path = self.models_dir.join(coreml.extracted_dirname);

        if extracted_path.exists() {
            debug!(path = %extracted_path.display(), "CoreML encoder already exists");
            return Ok(());
        }

        let zip_path = self.models_dir.join(coreml.zip_filename);

        // Download if zip doesn't exist
        if !zip_path.exists() {
            info!(
                url = %coreml.url,
                dest = %zip_path.display(),
                "Downloading CoreML encoder model"
            );

            let zip_info = ZipModelInfo {
                filename: coreml.zip_filename,
                url: &coreml.url,
            };
            self.download_zip_model(&zip_info, &zip_path).await?;
        }

        // Extract the zip
        info!(
            zip = %zip_path.display(),
            dest = %extracted_path.display(),
            "Extracting CoreML encoder model"
        );

        let status = Command::new("unzip")
            .args(["-q", "-o"])
            .arg(&zip_path)
            .arg("-d")
            .arg(&self.models_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .status()
            .await
            .context("Failed to run unzip command")?;

        if !status.success() {
            anyhow::bail!(
                "Failed to extract CoreML model: unzip exited with {}",
                status
            );
        }

        // Remove the zip file to save space
        fs::remove_file(&zip_path)
            .await
            .context("Failed to remove CoreML zip file")?;

        info!(path = %extracted_path.display(), "CoreML encoder model ready");

        Ok(())
    }

    /// Download a model from its URL with progress bar and resume support.
    async fn download_model(&self, info: &ModelInfo, dest: &Path) -> Result<()> {
        // Ensure directory exists
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create models directory")?;
        }

        let temp_path = dest.with_extension("tmp");

        // Check for existing partial download
        let existing_size = if temp_path.exists() {
            let metadata = fs::metadata(&temp_path)
                .await
                .context("Failed to read partial download metadata")?;
            metadata.len()
        } else {
            0
        };

        // Get total size for progress bar
        let total_size = info.size_bytes.unwrap_or(0);

        // If we already have the complete file, just validate and rename
        if existing_size > 0 && info.size_bytes == Some(existing_size) {
            info!(
                path = %temp_path.display(),
                size = existing_size,
                "Found complete partial download, finalizing"
            );
            fs::rename(&temp_path, dest)
                .await
                .context("Failed to finalize model file")?;
            return Ok(());
        }

        info!(
            url = %info.url,
            dest = %dest.display(),
            resuming_from = existing_size,
            "Downloading model"
        );

        // Build request with Range header for resume
        let client = reqwest::Client::new();
        let mut request = client.get(&info.url);

        if existing_size > 0 {
            info!(
                bytes_downloaded = existing_size,
                "Resuming download from byte {}", existing_size
            );
            request = request.header("Range", format!("bytes={}-", existing_size));
        }

        let response = request
            .send()
            .await
            .with_context(|| format!("Failed to download model from {}", info.url))?;

        let status = response.status();
        debug!(
            status = %status,
            url = %response.url(),
            "Received response"
        );

        // Handle 416 Range Not Satisfiable - delete partial and retry from scratch
        if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
            warn!("Server rejected range request (416), restarting download from scratch");
            let _ = fs::remove_file(&temp_path).await;
            // Recursive call without the partial file
            return Box::pin(self.download_model(info, dest)).await;
        }

        if !status.is_success() && status != reqwest::StatusCode::PARTIAL_CONTENT {
            anyhow::bail!(
                "Failed to download model: HTTP {} from {}",
                status,
                response.url()
            );
        }

        // Check if server supports range requests
        let is_resume = status == reqwest::StatusCode::PARTIAL_CONTENT;
        let downloaded_start = if is_resume { existing_size } else { 0 };

        // If server doesn't support resume and we have partial data, start fresh
        if !is_resume && existing_size > 0 {
            warn!("Server doesn't support resume, starting download from scratch");
            let _ = fs::remove_file(&temp_path).await;
        }

        // Set up progress bar
        let pb = ProgressBar::new(total_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
                .expect("Invalid progress template")
                .progress_chars("#>-"),
        );
        pb.set_message(format!("Downloading {}", info.filename));
        pb.set_position(downloaded_start);

        // Open file for writing (append if resuming)
        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(is_resume)
            .truncate(!is_resume)
            .open(&temp_path)
            .await
            .context("Failed to open temporary model file")?;

        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = downloaded_start;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Error reading download stream")?;
            file.write_all(&chunk)
                .await
                .context("Failed to write chunk")?;
            downloaded += chunk.len() as u64;
            pb.set_position(downloaded);
        }

        file.sync_all().await.context("Failed to sync model file")?;
        drop(file);

        // Validate size if known
        if let Some(expected) = info.size_bytes {
            if downloaded != expected {
                // Keep partial download for potential resume
                pb.abandon_with_message(format!(
                    "Download incomplete: got {} of {} bytes (will resume on next attempt)",
                    downloaded, expected
                ));
                anyhow::bail!(
                    "Downloaded model size mismatch: expected {}, got {} (partial download saved for resume)",
                    expected,
                    downloaded
                );
            }
        }

        // Atomic rename
        fs::rename(&temp_path, dest)
            .await
            .context("Failed to finalize model file")?;

        pb.finish_with_message(format!("Downloaded {}", info.filename));

        info!(
            path = %dest.display(),
            size = downloaded,
            "Model downloaded successfully"
        );

        Ok(())
    }

    /// Download a zip model file with progress bar.
    #[cfg(target_os = "macos")]
    async fn download_zip_model(&self, info: &ZipModelInfo<'_>, dest: &Path) -> Result<()> {
        // Ensure directory exists
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create models directory")?;
        }

        let temp_path = dest.with_extension("tmp");

        info!(
            url = %info.url,
            dest = %dest.display(),
            "Downloading CoreML model"
        );

        let client = reqwest::Client::new();
        let response = client
            .get(info.url)
            .send()
            .await
            .with_context(|| format!("Failed to download model from {}", info.url))?;

        let status = response.status();
        if !status.is_success() {
            anyhow::bail!(
                "Failed to download model: HTTP {} from {}",
                status,
                response.url()
            );
        }

        // Get content length for progress bar
        let total_size = response.content_length().unwrap_or(0);

        // Set up progress bar
        let pb = ProgressBar::new(total_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
                .expect("Invalid progress template")
                .progress_chars("#>-"),
        );
        pb.set_message(format!("Downloading {}", info.filename));

        // Download to temp file
        let mut file = fs::File::create(&temp_path)
            .await
            .context("Failed to create temporary model file")?;

        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Error reading download stream")?;
            file.write_all(&chunk)
                .await
                .context("Failed to write chunk")?;
            downloaded += chunk.len() as u64;
            pb.set_position(downloaded);
        }

        file.sync_all().await.context("Failed to sync model file")?;
        drop(file);

        // Atomic rename
        fs::rename(&temp_path, dest)
            .await
            .context("Failed to finalize model file")?;

        pb.finish_with_message(format!("Downloaded {}", info.filename));

        info!(
            path = %dest.display(),
            size = downloaded,
            "CoreML model downloaded successfully"
        );

        Ok(())
    }
}

/// Helper struct for zip model downloads.
#[cfg(target_os = "macos")]
struct ZipModelInfo<'a> {
    filename: &'a str,
    url: &'a str,
}

impl Default for ModelManager {
    fn default() -> Self {
        Self::new().expect("Failed to create default ModelManager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_model_info() {
        let info = ModelId::SileroVad.info();
        assert_eq!(info.filename, "silero_vad.onnx");
        assert!(info.url.contains("silero"));
    }

    #[test]
    fn test_model_manager_custom_dir() {
        let temp = TempDir::new().unwrap();
        let manager = ModelManager::with_dir(temp.path());
        assert_eq!(manager.models_dir(), temp.path());
    }

    #[test]
    fn test_model_path_construction() {
        let temp = TempDir::new().unwrap();
        let manager = ModelManager::with_dir(temp.path());

        // Model doesn't exist yet, so ensure_model would try to download
        // We just test the path would be correct
        let expected_path = temp.path().join("silero_vad.onnx");
        assert!(!expected_path.exists());
    }
}
