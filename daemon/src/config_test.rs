use super::*;
use tempfile::TempDir;

#[test]
fn test_default_config_values() {
    let config = Config::default();

    // Model defaults
    assert_eq!(config.model.model, SpeechModel::WhisperBase);
    assert_eq!(config.model.language, "auto");

    // Latency defaults
    assert_eq!(config.latency.mode, LatencyMode::Balanced);
    assert!((config.latency.min_chunk_seconds - 1.0).abs() < f32::EPSILON);

    // Injection defaults
    assert!(config.injection.allowlist.is_empty());
}

#[test]
fn test_load_valid_config_from_file() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    let toml_content = r#"
[model]
model = "whisper-base-en"
language = "en"

[latency]
mode = "fast"
min_chunk_seconds = 0.5

[injection]
allowlist = ["kitty", "alacritty"]
"#;

    std::fs::write(&config_path, toml_content).unwrap();

    let config = Config::load_from(&config_path).unwrap();

    assert_eq!(config.model.model, SpeechModel::WhisperBaseEn);
    assert_eq!(config.model.language, "en");
    assert_eq!(config.latency.mode, LatencyMode::Fast);
    assert!((config.latency.min_chunk_seconds - 0.5).abs() < f32::EPSILON);
    assert_eq!(
        config.injection.allowlist,
        vec!["kitty".to_string(), "alacritty".to_string()]
    );
}

#[test]
fn test_missing_config_file_returns_defaults() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("nonexistent.toml");

    let config = Config::load_from(&config_path).unwrap();

    assert_eq!(config, Config::default());
}

#[test]
fn test_invalid_toml_returns_error() {
    let invalid_toml = "this is not valid { toml [";

    let result = Config::parse(invalid_toml);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("TOML"));
}

#[test]
fn test_invalid_model_name_returns_error() {
    let toml_content = r#"
[model]
model = "not-a-real-model"
"#;

    let result = Config::parse(toml_content);
    assert!(result.is_err());
}

#[test]
fn test_partial_config_uses_defaults_for_missing() {
    let partial_toml = r#"
[model]
model = "whisper-tiny"
"#;

    let config = Config::parse(partial_toml).unwrap();

    // Specified value
    assert_eq!(config.model.model, SpeechModel::WhisperTiny);
    // Default values for unspecified fields
    assert_eq!(config.model.language, "auto");
    assert_eq!(config.latency.mode, LatencyMode::Balanced);
    assert!(config.injection.allowlist.is_empty());
}

#[test]
fn test_config_paths() {
    // These should return valid paths on any system
    let config_dir = Config::config_dir().unwrap();
    let config_path = Config::config_path().unwrap();
    let data_dir = Config::data_dir().unwrap();
    let models_dir = Config::models_dir().unwrap();

    assert!(config_dir.ends_with("voice-controllm"));
    assert!(config_path.ends_with("config.toml"));
    assert!(data_dir.ends_with("voice-controllm"));
    assert!(models_dir.ends_with("models"));

    // Verify parent relationships
    assert_eq!(config_path.parent().unwrap(), config_dir);
    assert_eq!(models_dir.parent().unwrap(), data_dir);
}

#[test]
fn test_save_and_load_roundtrip() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");

    let original = Config {
        model: ModelConfig {
            model: SpeechModel::WhisperMedium,
            language: "cs".to_string(),
        },
        latency: LatencyConfig {
            mode: LatencyMode::Accurate,
            min_chunk_seconds: 2.0,
        },
        injection: InjectionConfig {
            allowlist: vec!["IntelliJ IDEA".to_string()],
        },
        logging: LoggingConfig {
            level: LogLevel::Debug,
        },
        gui: GuiConfig {
            languages: vec!["en".to_string(), "cs".to_string()],
        },
    };

    original.save_to(&config_path).unwrap();
    let loaded = Config::load_from(&config_path).unwrap();

    assert_eq!(original, loaded);
}

#[test]
fn test_save_creates_parent_directories() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("nested/dir/config.toml");

    let config = Config::default();
    config.save_to(&config_path).unwrap();

    assert!(config_path.exists());
}

#[test]
fn test_latency_mode_serialization() {
    // Test that modes serialize to lowercase
    let config = Config {
        latency: LatencyConfig {
            mode: LatencyMode::Fast,
            ..Default::default()
        },
        ..Default::default()
    };

    let toml_str = toml::to_string(&config).unwrap();
    assert!(toml_str.contains("mode = \"fast\""));
}

#[test]
fn test_speech_model_serialization() {
    let config = Config {
        model: ModelConfig {
            model: SpeechModel::WhisperBase,
            ..Default::default()
        },
        ..Default::default()
    };

    let toml_str = toml::to_string(&config).unwrap();
    assert!(toml_str.contains("model = \"whisper-base\""));
}

#[test]
fn test_empty_allowlist_not_serialized() {
    let config = Config::default();
    let toml_str = toml::to_string(&config).unwrap();

    // Empty allowlist should be omitted from output
    assert!(!toml_str.contains("allowlist"));
}

#[test]
fn test_language_auto_detection() {
    let toml_content = r#"
[model]
language = "auto"
"#;

    let config = Config::parse(toml_content).unwrap();
    assert_eq!(config.model.language, "auto");
}

#[test]
fn test_language_specific() {
    let toml_content = r#"
[model]
language = "slovak"
"#;

    let config = Config::parse(toml_content).unwrap();
    assert_eq!(config.model.language, "slovak");
}

#[test]
fn gui_languages_parsed() {
    let toml = r#"
[gui]
languages = ["en", "cs", "de"]
"#;
    let config: Config = toml::from_str(toml).unwrap();
    assert_eq!(config.gui.languages, vec!["en", "cs", "de"]);
}

#[test]
fn gui_defaults_to_empty_languages() {
    let config: Config = toml::from_str("").unwrap();
    assert!(config.gui.languages.is_empty());
}
