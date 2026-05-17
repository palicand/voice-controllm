use super::*;

#[test]
fn test_is_allowed_empty_allowlist() {
    let config = InjectionConfig::default();
    let injector = KeystrokeInjector::new(config).expect("should create injector");

    // Empty allowlist means all apps are allowed (checked before is_allowed)
    assert!(injector.config.allowlist.is_empty());
}

#[test]
fn test_is_allowed_case_insensitive() {
    let config = InjectionConfig {
        allowlist: vec!["Terminal".to_string(), "VSCode".to_string()],
    };
    let injector = KeystrokeInjector::new(config).expect("should create injector");

    assert!(injector.is_allowed("Terminal"));
    assert!(injector.is_allowed("terminal"));
    assert!(injector.is_allowed("TERMINAL"));
    assert!(injector.is_allowed("VSCode"));
    assert!(injector.is_allowed("vscode"));
    assert!(!injector.is_allowed("Safari"));
}

#[test]
fn test_is_allowed_partial_match() {
    let config = InjectionConfig {
        allowlist: vec!["Code".to_string()],
    };
    let injector = KeystrokeInjector::new(config).expect("should create injector");

    // Partial match: "Visual Studio Code" contains "Code"
    assert!(injector.is_allowed("Visual Studio Code"));
    assert!(injector.is_allowed("code"));
    assert!(!injector.is_allowed("Terminal"));
}

#[cfg(target_os = "macos")]
#[test]
fn test_get_frontmost_app() {
    if let Ok(app) = vcm_platform::frontmost::current() {
        assert!(!app.is_empty());
    }
}
