use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use crate::managed_file::digest_file;
use crate::state::StateResource;

const UNIT_DIR: &str = "/etc/systemd/system";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SystemdUnitResource {
    pub(crate) unit: String,
    pub(crate) file: PathBuf,
}

pub(crate) fn systemd_unit_id_for(resource: &SystemdUnitResource) -> String {
    format!("systemd-unit:{}", resource.unit)
}

pub(crate) fn systemd_available() -> bool {
    Command::new("systemctl")
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub(crate) fn unit_installed(resource: &SystemdUnitResource) -> bool {
    installed_path(resource).exists()
}

pub(crate) fn unit_file_matches(resource: &SystemdUnitResource) -> Result<bool> {
    let source = fs::read(&resource.file)?;
    let installed = match fs::read(installed_path(resource)) {
        Ok(installed) => installed,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    Ok(source == installed)
}

pub(crate) fn unit_current(resource: &SystemdUnitResource) -> Result<bool> {
    unit_file_matches(resource)
}

pub(crate) fn apply_unit(resource: &SystemdUnitResource) -> Result<()> {
    let installed = installed_path(resource);
    run_sudo(
        ["install", "-m", "0644"],
        [resource.file.as_path(), installed.as_path()],
    )?;
    run_sudo(["systemctl", "daemon-reload"], std::iter::empty::<&Path>())?;
    if !unit_current(resource)? {
        bail!(
            "systemd service {} is not current after apply",
            resource.unit
        );
    }
    Ok(())
}

pub(crate) fn unit_safe_to_remove(
    resource: &SystemdUnitResource,
    digest: Option<&str>,
) -> Result<bool> {
    let installed = installed_path(resource);
    if !installed.exists() {
        return Ok(true);
    }
    match digest {
        Some(expected) => Ok(digest_file(&installed)? == expected),
        None if resource.file.exists() => unit_file_matches(resource),
        None => Ok(false),
    }
}

pub(crate) fn remove_unit(resource: &SystemdUnitResource, digest: Option<&str>) -> Result<()> {
    if !unit_safe_to_remove(resource, digest)? {
        bail!(
            "refusing to remove systemd unit changed outside dots: {}",
            resource.unit
        );
    }
    run_sudo(["rm", "-f"], [installed_path(resource).as_path()])?;
    run_sudo(["systemctl", "daemon-reload"], std::iter::empty::<&Path>())
}

pub(crate) fn state_systemd_unit(resource: &SystemdUnitResource) -> Result<StateResource> {
    Ok(StateResource::SystemdUnit {
        unit: resource.unit.clone(),
        file: resource.file.clone(),
        digest: Some(digest_file(&installed_path(resource))?),
    })
}

pub(crate) fn systemd_unit_from_state(
    resource: &StateResource,
) -> Option<(SystemdUnitResource, Option<&str>)> {
    let StateResource::SystemdUnit { unit, file, digest } = resource else {
        return None;
    };
    Some((
        SystemdUnitResource {
            unit: unit.clone(),
            file: file.clone(),
        },
        digest.as_deref(),
    ))
}

fn installed_path(resource: &SystemdUnitResource) -> PathBuf {
    std::env::var_os("DOTS_SYSTEMD_UNIT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(UNIT_DIR))
        .join(&resource.unit)
}

fn run_sudo<'a>(
    args: impl IntoIterator<Item = &'a str>,
    paths: impl IntoIterator<Item = &'a Path>,
) -> Result<()> {
    let status = Command::new("sudo")
        .args(args)
        .args(paths)
        .status()
        .with_context(|| "failed to run sudo")?;
    if !status.success() {
        bail!("sudo command failed");
    }
    Ok(())
}
