use anyhow::Result;

pub trait Autostart {
    fn enable(&self) -> Result<()>;
    fn disable(&self) -> Result<()>;
    fn is_enabled(&self) -> Result<bool>;
}

#[cfg(target_os = "macos")]
pub fn default_backend() -> Result<Box<dyn Autostart>> {
    Ok(Box::new(
        crate::macos::autostart::LaunchAgent::for_current_exe()?,
    ))
}

#[cfg(not(target_os = "macos"))]
pub fn default_backend() -> Result<Box<dyn Autostart>> {
    Ok(Box::new(NoopAutostart))
}

#[cfg(not(target_os = "macos"))]
struct NoopAutostart;

#[cfg(not(target_os = "macos"))]
impl Autostart for NoopAutostart {
    fn enable(&self) -> Result<()> {
        Ok(())
    }
    fn disable(&self) -> Result<()> {
        Ok(())
    }
    fn is_enabled(&self) -> Result<bool> {
        Ok(false)
    }
}
