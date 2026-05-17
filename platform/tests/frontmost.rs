use vcm_platform::frontmost::{self, FrontmostApp};

#[cfg(target_os = "macos")]
#[test]
#[ignore] // requires running window server
fn current_returns_non_empty_when_focus_app_exists() {
    let name = frontmost::current().expect("frontmost query");
    // A windowed session always has *some* frontmost app — even Finder.
    assert!(!name.is_empty(), "expected non-empty frontmost app name");
}

#[test]
fn trait_is_object_safe_for_dyn_dispatch_compile_check() {
    // Compile-time only: ensures the trait can be boxed.
    fn _accepts(_x: Box<dyn FrontmostApp>) {}
}
