use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub(crate) enum StateResource {
    #[serde(rename = "symlink")]
    Symlink { target: PathBuf, source: PathBuf },
    #[serde(rename = "package")]
    Package { provider: String, name: String },
    #[serde(rename = "service")]
    Service {
        provider: String,
        action: String,
        name: String,
    },
    #[serde(rename = "font")]
    Font { source: PathBuf, target: PathBuf },
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct State {
    pub(crate) resources: BTreeMap<String, StateResource>,
}

pub(crate) fn load_state(path: &Path) -> Result<State> {
    let Some(source) = fs::read_to_string(path).ok() else {
        return Ok(State::default());
    };
    serde_json::from_str(&source).with_context(|| format!("failed to parse {}", path.display()))
}

pub(crate) fn save_state(path: &Path, state: &State) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(state)?)?;
    fs::rename(tmp, path)?;
    Ok(())
}
