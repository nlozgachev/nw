use crate::error::NwError;
use crate::model::Portfolio;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

fn non_empty_os(value: Option<&OsStr>) -> Option<&OsStr> {
    value.filter(|s| !s.is_empty())
}

/// Pure path resolver for the default `portfolio.json` location.
pub fn resolve_portfolio_path(
    xdg_config_home: Option<&OsStr>,
    home: Option<&OsStr>,
    appdata: Option<&OsStr>,
    userprofile: Option<&OsStr>,
) -> Result<PathBuf, NwError> {
    let config_dir = if let Some(xdg) = non_empty_os(xdg_config_home) {
        PathBuf::from(xdg)
    } else if cfg!(windows) {
        if let Some(app) = non_empty_os(appdata) {
            PathBuf::from(app)
        } else if let Some(profile) = non_empty_os(userprofile) {
            PathBuf::from(profile).join(".config")
        } else if let Some(h) = non_empty_os(home) {
            PathBuf::from(h).join(".config")
        } else {
            return Err(NwError::NoConfigDir);
        }
    } else if let Some(h) = non_empty_os(home) {
        PathBuf::from(h).join(".config")
    } else {
        return Err(NwError::NoConfigDir);
    };

    Ok(config_dir.join("nw-tracker").join("portfolio.json"))
}

/// Determine the default `portfolio.json` path from the environment.
pub fn default_portfolio_path() -> Result<PathBuf, NwError> {
    resolve_portfolio_path(
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
        std::env::var_os("APPDATA").as_deref(),
        std::env::var_os("USERPROFILE").as_deref(),
    )
}

#[derive(Debug, Clone)]
pub struct PortfolioStore {
    path: PathBuf,
}

impl PortfolioStore {
    pub const fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn from_override(file_override: Option<PathBuf>) -> Result<Self, NwError> {
        let path = match file_override {
            Some(p) => p,
            None => default_portfolio_path()?,
        };
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<Portfolio, NwError> {
        let contents = match fs::read_to_string(&self.path) {
            Ok(c) => c,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Portfolio::default()),
            Err(e) => {
                return Err(NwError::ReadFile {
                    path: self.path.clone(),
                    source: e,
                });
            }
        };

        let mut portfolio: Portfolio =
            serde_json::from_str(&contents).map_err(|e| NwError::MalformedJson {
                path: self.path.clone(),
                source: e,
            })?;
        portfolio.validate()?;
        portfolio.sort_snapshots();
        Ok(portfolio)
    }

    pub fn save(&self, portfolio: &mut Portfolio) -> Result<(), NwError> {
        portfolio.sort_snapshots();
        portfolio.validate()?;

        let parent = self
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));

        fs::create_dir_all(parent).map_err(|e| NwError::WriteFile {
            path: parent.to_path_buf(),
            source: e,
        })?;

        let mut contents =
            serde_json::to_string_pretty(portfolio).map_err(|e| NwError::SerializeJson {
                path: self.path.clone(),
                source: e,
            })?;
        contents.push('\n');

        let mut tmp = NamedTempFile::new_in(parent).map_err(|e| NwError::WriteFile {
            path: parent.to_path_buf(),
            source: e,
        })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o600);
            tmp.as_file()
                .set_permissions(perms)
                .map_err(|e| NwError::WriteFile {
                    path: tmp.path().to_path_buf(),
                    source: e,
                })?;
        }

        tmp.write_all(contents.as_bytes())
            .map_err(|e| NwError::WriteFile {
                path: tmp.path().to_path_buf(),
                source: e,
            })?;

        tmp.as_file().sync_all().map_err(|e| NwError::WriteFile {
            path: tmp.path().to_path_buf(),
            source: e,
        })?;

        tmp.persist(&self.path).map_err(|e| NwError::WriteFile {
            path: self.path.clone(),
            source: e.error,
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests;
