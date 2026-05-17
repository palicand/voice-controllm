use crate::config::InjectionConfig;
use anyhow::{Context, Result};
use enigo::{Enigo, Keyboard, Settings};
use tracing::{debug, info, warn};

pub struct KeystrokeInjector {
    config: InjectionConfig,
    enigo: Enigo,
}

impl KeystrokeInjector {
    pub fn new(config: InjectionConfig) -> Result<Self> {
        let enigo = Enigo::new(&Settings::default())
            .map_err(anyhow::Error::new)
            .context("initialize enigo")?;

        Ok(Self { config, enigo })
    }

    pub fn inject_text(&mut self, text: &str) -> Result<()> {
        if !self.config.allowlist.is_empty() {
            let frontmost = vcm_platform::frontmost::current().unwrap_or_else(|e| {
                warn!(error = %e, "Failed to get frontmost app, skipping allowlist check");
                String::new()
            });

            if !frontmost.is_empty() && !self.is_allowed(&frontmost) {
                debug!(
                    app = %frontmost,
                    "Skipping injection: app not in allowlist"
                );
                return Ok(());
            }
        }

        info!(text = %text, "Injecting text as keystrokes");
        self.enigo
            .text(text)
            .map_err(anyhow::Error::new)
            .context("inject text")?;

        Ok(())
    }

    fn is_allowed(&self, app_name: &str) -> bool {
        let app_lower = app_name.to_lowercase();
        self.config
            .allowlist
            .iter()
            .any(|allowed| app_lower.contains(&allowed.to_lowercase()))
    }
}

#[cfg(test)]
#[path = "inject_test.rs"]
mod tests;
