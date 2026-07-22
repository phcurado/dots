use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::service::ServiceAction;

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
        action: ServiceAction,
        name: String,
    },
    #[serde(rename = "systemd-unit")]
    SystemdUnit {
        unit: String,
        file: PathBuf,
        #[serde(default)]
        digest: Option<String>,
    },
    #[serde(rename = "compose")]
    Compose {
        name: String,
        file: PathBuf,
        profiles: Vec<String>,
        apply: Vec<String>,
        remove: Vec<String>,
        fingerprint: String,
    },
    #[serde(rename = "font")]
    Font {
        source: PathBuf,
        target: PathBuf,
        #[serde(default)]
        digest: Option<String>,
    },
    #[serde(rename = "file")]
    File {
        target: PathBuf,
        source: PathBuf,
        source_digest: String,
        mode: Option<u32>,
    },
    #[serde(rename = "ssh-keypair")]
    SshKeypair {
        name: String,
        private_path: PathBuf,
        public_path: PathBuf,
        private_digest: String,
        public_digest: String,
        fingerprint: String,
        public_key: String,
        encrypted: bool,
    },
    #[serde(rename = "group")]
    Group { name: String },
    #[serde(rename = "user-group")]
    UserGroup { name: String },
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct State {
    pub(crate) resources: BTreeMap<String, StateResource>,
    #[serde(default)]
    pub(crate) outputs: BTreeMap<String, serde_json::Value>,
}

impl StateResource {
    pub(crate) const KEY_PREFIXES: &'static [&'static str] = &[
        "symlink:",
        "package:",
        "service:",
        "systemd-unit:",
        "compose:",
        "font:",
        "file:",
        "ssh-keypair:",
        "group:",
        "user-group:",
    ];
}

pub(crate) fn load_state(path: &Path) -> Result<State> {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(State::default()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()));
        }
    };
    serde_json::from_str(&source).with_context(|| {
        format!(
            "failed to parse {}; delete this file to reset local state",
            path.display()
        )
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_state_loads_as_empty() {
        let root = tempfile::tempdir().unwrap();

        let state = load_state(&root.path().join("missing.json")).unwrap();

        assert!(state.resources.is_empty());
    }

    #[test]
    fn load_state_returns_read_errors() {
        let root = tempfile::tempdir().unwrap();

        let error = load_state(root.path()).unwrap_err().to_string();

        assert!(error.contains("failed to read"));
    }

    #[test]
    fn older_resources_load_without_digests() {
        let state: State = serde_json::from_str(
            r#"{
                "resources": {
                    "font:/tmp/font": {
                        "kind": "font",
                        "source": "/tmp/source",
                        "target": "/tmp/font"
                    },
                    "systemd-unit:example.service": {
                        "kind": "systemd-unit",
                        "unit": "example.service",
                        "file": "/tmp/example.service"
                    }
                }
            }"#,
        )
        .unwrap();

        assert!(matches!(
            state.resources["font:/tmp/font"],
            StateResource::Font { digest: None, .. }
        ));
        assert!(matches!(
            state.resources["systemd-unit:example.service"],
            StateResource::SystemdUnit { digest: None, .. }
        ));
    }

    #[test]
    fn corrupt_state_parse_error_has_reset_hint() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("state.json");
        fs::write(&path, "not json").unwrap();

        let error = load_state(&path).unwrap_err().to_string();

        assert!(error.contains("failed to parse"));
        assert!(error.contains("delete this file to reset local state"));
    }
}
