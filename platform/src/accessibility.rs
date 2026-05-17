#[cfg(target_os = "macos")]
pub fn is_trusted_or_prompt() -> bool {
    crate::macos::accessibility::is_trusted_or_prompt()
}

#[cfg(not(target_os = "macos"))]
pub fn is_trusted_or_prompt() -> bool {
    true
}
