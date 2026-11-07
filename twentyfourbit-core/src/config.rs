//! Configuration file handling.
//!
//! Lives at `~/.config/twentyfourbit/config.toml` by default. Command-line
//! flags always win over this file; the file exists so a token and a preferred
//! quality tier survive between invocations.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::model::Quality;

/// User configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Preferred quality tier.
    pub quality: Quality,
    /// Bearer token from [`crate::Client::login`].
    pub token: Option<String>,
    /// Placeholder sent in the `ip` field the site demands.
    pub ip_placeholder: String,
    /// Directory downloads are saved into when no path is given.
    pub download_dir: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            quality: Quality::Master,
            token: None,
            ip_placeholder: crate::client::IP_PLACEHOLDER.to_owned(),
            download_dir: None,
        }
    }
}

impl Config {
    /// The default config path, honouring `XDG_CONFIG_HOME`.
    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("twentyfourbit")
            .join("config.toml")
    }

    /// Load from the default path, falling back to defaults when absent.
    pub fn load() -> Result<Self> {
        Self::load_from(&Self::default_path())
    }

    /// Load from a specific path. A missing file yields [`Config::default`].
    pub fn load_from(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|source| Error::Config {
                path: path.to_path_buf(),
                source,
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(Error::Io {
                path: path.to_path_buf(),
                source,
            }),
        }
    }

    /// Persist to the default path, creating parent directories.
    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::default_path())
    }

    /// Persist to a specific path.
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Error::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let text = toml::to_string(self).map_err(|source| Error::ConfigEncode {
            path: path.to_path_buf(),
            source,
        })?;
        std::fs::write(path, text).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })
    }
}
