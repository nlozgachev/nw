use std::fs;
use std::path::PathBuf;
use crate::error::NwError;
use crate::model::Portfolio;

pub fn portfolio_path() -> Result<PathBuf, NwError> {
    let config_dir = dirs_next().ok_or(NwError::NoConfigDir)?;
    Ok(config_dir.join("nw-tracker").join("portfolio.json"))
}

fn dirs_next() -> Option<PathBuf> {
    // Use $HOME/.config on Unix (XDG convention)
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(xdg));
    }
    dirs_home().map(|h| h.join(".config"))
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

pub fn load_portfolio() -> Result<Portfolio, NwError> {
    let path = portfolio_path()?;

    if !path.exists() {
        return Ok(Portfolio::default());
    }

    let contents = fs::read_to_string(&path).map_err(|e| NwError::ReadFile {
        path: path.display().to_string(),
        source: e,
    })?;

    serde_json::from_str(&contents).map_err(|e| NwError::MalformedJson {
        path: path.display().to_string(),
        source: e,
    })
}

pub fn save_portfolio(portfolio: &mut Portfolio) -> Result<(), NwError> {
    let path = portfolio_path()?;

    // Enforce ascending date sort — single enforcement point
    portfolio.snapshots.sort_by(|a, b| a.date.cmp(&b.date));

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| NwError::WriteFile {
            path: parent.display().to_string(),
            source: e,
        })?;
    }

    let contents = serde_json::to_string_pretty(portfolio).map_err(|e| NwError::SerializeJson {
        path: path.display().to_string(),
        source: e,
    })?;

    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, &contents).map_err(|e| NwError::WriteFile {
        path: tmp_path.display().to_string(),
        source: e,
    })?;
    fs::rename(&tmp_path, &path).map_err(|e| NwError::WriteFile {
        path: path.display().to_string(),
        source: e,
    })
}
