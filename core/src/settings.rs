use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};

static CONFIG_DIR_OVERRIDE: OnceCell<PathBuf> = OnceCell::new();

/// Configure the persistent storage directory for foreign platforms such as Android.
/// Desktop callers continue to use the platform config directory by default.
pub fn configure_config_dir(path: String) -> Result<()> {
    let path = PathBuf::from(path);
    if path.as_os_str().is_empty() {
        bail!("config directory must not be empty");
    }
    fs::create_dir_all(&path)
        .with_context(|| format!("failed to create config directory {}", path.display()))?;

    if let Some(existing) = CONFIG_DIR_OVERRIDE.get() {
        if existing != &path {
            bail!("config directory is already configured");
        }
        return Ok(());
    }

    let _ = CONFIG_DIR_OVERRIDE.set(path);
    Ok(())
}

pub fn config_dir() -> Result<PathBuf> {
    CONFIG_DIR_OVERRIDE
        .get()
        .cloned()
        .or_else(dirs::config_dir)
        .context("no config directory for this user")
}

/// Allowed values for `Settings::device_type` on the wire (protocol v1).
pub const DEVICE_TYPES: [&str; 2] = ["phone", "desktop"];

/// How incoming pairing requests are confirmed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confirmation {
    /// Ask the user (stdin for the daemon, UI callback for apps).
    #[default]
    Prompt,
    /// Accept every pairing request without asking. Only for kiosks/testing.
    Auto,
}

/// Persistent daemon configuration, loaded from `config.toml`.
///
/// Defaults are also used by the UniFFI entry point, where values are passed
/// in as arguments instead of read from disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Name advertised over mDNS and shown to peers. Defaults to the host name.
    pub device_name: String,
    /// `"phone"` or `"desktop"`.
    pub device_type: String,
    /// Bind address for the incoming TCP listener.
    pub listen_host: String,
    /// Bind port. `0` picks an ephemeral port, reported back over mDNS.
    pub listen_port: u16,
    /// `true`: dial and pair with discovered devices automatically.
    pub auto_pair: bool,
    pub confirmation: Confirmation,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            device_name: default_device_name(),
            device_type: "desktop".to_string(),
            listen_host: "0.0.0.0".to_string(),
            listen_port: 0,
            auto_pair: true,
            confirmation: Confirmation::Prompt,
        }
    }
}

fn default_device_name() -> String {
    hostname::get()
        .ok()
        .and_then(|name| name.to_str().map(str::to_owned))
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "HyprConnect Device".to_string())
}

impl Settings {
    /// Path of the configuration file: `<config_dir>/hyprconnect/config.toml`.
    pub fn config_path() -> Result<PathBuf> {
        let dir = config_dir()?;
        Ok(dir.join("hyprconnect").join("config.toml"))
    }

    /// Load the configuration file, creating it with defaults on first run.
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            let settings = Self::default();
            settings.write(&path)?;
            return Ok(settings);
        }

        let text = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let settings: Self =
            toml::from_str(&text).with_context(|| format!("invalid {}", path.display()))?;
        settings.validate()?;
        Ok(settings)
    }

    /// Build settings from the values handed over the FFI boundary.
    ///
    /// The config file is intentionally not consulted here: callers pass
    /// explicit values, so Android can advertise `Build.MODEL` without
    /// needing a config file on the device.
    pub fn from_args(device_name: &str, device_type: &str, auto_pair: bool) -> Result<Self> {
        let settings = Self {
            device_name: device_name.trim().to_string(),
            device_type: device_type.trim().to_lowercase(),
            auto_pair,
            ..Self::default()
        };
        settings.validate()?;
        Ok(settings)
    }

    /// Reject values that would break discovery or the peer's UI.
    pub fn validate(&self) -> Result<()> {
        if self.device_name.trim().is_empty() {
            bail!("device_name must not be empty");
        }
        if !DEVICE_TYPES.contains(&self.device_type.as_str()) {
            bail!(
                "device_type must be one of {}, got {:?}",
                DEVICE_TYPES.join(", "),
                self.device_type
            );
        }
        if self.listen_host.trim().is_empty() {
            bail!("listen_host must not be empty");
        }
        Ok(())
    }

    fn write(&self, path: &PathBuf) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let mut document = String::from(
            "# HyprConnect settings.\n\
             # device_type: \"phone\" or \"desktop\"\n\
             # listen_port: 0 picks an ephemeral port\n\
             # confirmation: \"prompt\" or \"auto\" (auto accepts pairing without asking)\n\n",
        );
        document.push_str(&toml::to_string_pretty(self)?);
        fs::write(path, document).with_context(|| format!("failed to write {}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        assert!(Settings::default().validate().is_ok());
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let parsed: Settings = toml::from_str("device_name = \"Box\"\n").unwrap();
        assert_eq!(parsed.device_name, "Box");
        assert_eq!(parsed.device_type, "desktop");
        assert_eq!(parsed.listen_port, 0);
        assert_eq!(parsed.confirmation, Confirmation::Prompt);
    }

    #[test]
    fn unknown_device_type_is_rejected() {
        let err = Settings::from_args("Box", "tablet", false).unwrap_err();
        assert!(err.to_string().contains("device_type"));
    }

    #[test]
    fn empty_device_name_is_rejected() {
        assert!(Settings::from_args("   ", "desktop", false).is_err());
    }

    #[test]
    fn device_type_is_lowercased() {
        let settings = Settings::from_args("Box", "Desktop", false).unwrap();
        assert_eq!(settings.device_type, "desktop");
    }
}
