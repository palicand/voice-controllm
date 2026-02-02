//! Model download and management.
//!
//! Handles automatic downloading of ML models on first run.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;
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
            },
            ModelId::WhisperTiny => ModelInfo {
                filename: "ggml-tiny.bin",
                url: format!("{}/ggml-tiny.bin", WHISPER_BASE_URL),
                size_bytes: Some(77_691_713),
            },
            ModelId::WhisperTinyEn => ModelInfo {
                filename: "ggml-tiny.en.bin",
                url: format!("{}/ggml-tiny.en.bin", WHISPER_BASE_URL),
                size_bytes: Some(77_704_715),
            },
            ModelId::WhisperBase => ModelInfo {
                filename: "ggml-base.bin",
                url: format!("{}/ggml-base.bin", WHISPER_BASE_URL),
                size_bytes: Some(147_951_465),
            },
            ModelId::WhisperBaseEn => ModelInfo {
                filename: "ggml-base.en.bin",
                url: format!("{}/ggml-base.en.bin", WHISPER_BASE_URL),
                size_bytes: Some(147_964_211),
            },
            ModelId::WhisperSmall => ModelInfo {
                filename: "ggml-small.bin",
                url: format!("{}/ggml-small.bin", WHISPER_BASE_URL),
                size_bytes: Some(487_601_967),
            },
            ModelId::WhisperSmallEn => ModelInfo {
                filename: "ggml-small.en.bin",
                url: format!("{}/ggml-small.en.bin", WHISPER_BASE_URL),
                size_bytes: Some(487_614_201),
            },
            ModelId::WhisperMedium => ModelInfo {
                filename: "ggml-medium.bin",
                url: format!("{}/ggml-medium.bin", WHISPER_BASE_URL),
                size_bytes: Some(1_533_774_781),
            },
            ModelId::WhisperMediumEn => ModelInfo {
                filename: "ggml-medium.en.bin",
                url: format!("{}/ggml-medium.en.bin", WHISPER_BASE_URL),
                size_bytes: Some(1_533_774_781),
            },
            ModelId::WhisperLargeV3 => ModelInfo {
                filename: "ggml-large-v3.bin",
                url: format!("{}/ggml-large-v3.bin", WHISPER_BASE_URL),
                size_bytes: Some(3_094_623_691),
            },
            ModelId::WhisperLargeV3Turbo => ModelInfo {
                filename: "ggml-large-v3-turbo.bin",
                url: format!("{}/ggml-large-v3-turbo.bin", WHISPER_BASE_URL),
                size_bytes: Some(1_624_592_891),
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
}

/// Manages model downloads and storage.
pub struct ModelManager {
    models_dir: PathBuf,
}

impl ModelManager {
    /// Create a new ModelManager using the default models directory.
    ///
    /// Default: `~/.local/share/voice-controllm/models/`
    pub fn new() -> Result<Self> {
        let models_dir = dirs::data_dir()
            .context("Could not determine data directory")?
            .join("voice-controllm")
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

        if model_path.exists() {
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
                } else {
                    debug!(path = %model_path.display(), "Model already exists");
                    return Ok(model_path);
                }
            } else {
                debug!(path = %model_path.display(), "Model already exists");
                return Ok(model_path);
            }
        }

        // Download the model
        self.download_model(&info, &model_path).await?;
        Ok(model_path)
    }

    /// Download a model from its URL.
    async fn download_model(&self, info: &ModelInfo, dest: &Path) -> Result<()> {
        // Ensure directory exists
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create models directory")?;
        }

        info!(
            url = %info.url,
            dest = %dest.display(),
            "Downloading model"
        );

        let response = reqwest::get(&info.url)
            .await
            .with_context(|| format!("Failed to download model from {}", info.url))?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to download model: HTTP {}", response.status());
        }

        let bytes = response
            .bytes()
            .await
            .context("Failed to read response body")?;

        if let Some(expected) = info.size_bytes {
            if bytes.len() as u64 != expected {
                anyhow::bail!(
                    "Downloaded model size mismatch: expected {}, got {}",
                    expected,
                    bytes.len()
                );
            }
        }

        // Write to temporary file first, then rename (atomic)
        let temp_path = dest.with_extension("tmp");
        let mut file = fs::File::create(&temp_path)
            .await
            .context("Failed to create temporary model file")?;
        file.write_all(&bytes)
            .await
            .context("Failed to write model file")?;
        file.sync_all().await.context("Failed to sync model file")?;

        fs::rename(&temp_path, dest)
            .await
            .context("Failed to finalize model file")?;

        info!(
            path = %dest.display(),
            size = bytes.len(),
            "Model downloaded successfully"
        );

        Ok(())
    }
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
