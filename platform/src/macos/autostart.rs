use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::autostart::Autostart;

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub struct LaunchAgent {
    label: String,
    program_path: PathBuf,
}

impl LaunchAgent {
    pub fn for_current_exe() -> Result<Self> {
        Ok(Self {
            label: crate::logging::LOG_SUBSYSTEM.to_string(),
            program_path: std::env::current_exe().context("resolve current exe for LaunchAgent")?,
        })
    }

    fn plist_path(&self) -> Result<PathBuf> {
        let home = dirs::home_dir().context("no home dir")?;
        Ok(home
            .join("Library/LaunchAgents")
            .join(format!("{}.plist", self.label)))
    }

    fn plist_body(&self) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>{}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key><true/>
    <key>KeepAlive</key><false/>
</dict>
</plist>
"#,
            self.label,
            xml_escape(&self.program_path.display().to_string())
        )
    }
}

impl Autostart for LaunchAgent {
    fn enable(&self) -> Result<()> {
        let path = self.plist_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("create LaunchAgents dir")?;
        }
        fs::write(&path, self.plist_body()).context("write LaunchAgent plist")?;
        Ok(())
    }

    fn disable(&self) -> Result<()> {
        let path = self.plist_path()?;
        if path.try_exists().context("stat LaunchAgent plist")? {
            fs::remove_file(&path).context("remove LaunchAgent plist")?;
        }
        Ok(())
    }

    fn is_enabled(&self) -> Result<bool> {
        self.plist_path()?
            .try_exists()
            .context("stat LaunchAgent plist")
    }
}
