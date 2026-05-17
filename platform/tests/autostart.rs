use vcm_platform::autostart::{self, Autostart};

#[cfg(target_os = "macos")]
#[test]
#[ignore] // touches real ~/Library/LaunchAgents
fn enable_then_is_enabled_then_disable() {
    let backend = autostart::default_backend().expect("default_backend");
    backend.disable().ok(); // start clean
    assert!(!backend.is_enabled().unwrap());
    backend.enable().expect("enable");
    assert!(backend.is_enabled().unwrap());
    backend.disable().expect("disable");
    assert!(!backend.is_enabled().unwrap());
}

#[test]
fn trait_is_object_safe() {
    fn _accepts(_x: Box<dyn Autostart>) {}
}
