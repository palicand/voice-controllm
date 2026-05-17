use anyhow::Result;

pub trait Autostart {
    fn enable(&self) -> Result<()>;
    fn disable(&self) -> Result<()>;
    fn is_enabled(&self) -> Result<bool>;
}

#[cfg(target_os = "macos")]
pub fn default_backend() -> Box<dyn Autostart> {
    Box::new(crate::macos::autostart::LaunchAgent::default())
}

#[cfg(not(target_os = "macos"))]
pub fn default_backend() -> Box<dyn Autostart> {
    Box::new(NoopAutostart)
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
