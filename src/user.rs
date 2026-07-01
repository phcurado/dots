use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::{Command as ProcessCommand, Stdio};

use anyhow::{Context, Result, bail};

#[derive(Debug, Default)]
pub(crate) struct UserConfig {
    pub(crate) shell: Option<UserShellResource>,
    pub(crate) groups: Vec<SystemGroupResource>,
    pub(crate) memberships: Vec<UserGroupResource>,
}

#[derive(Debug, Clone)]
pub(crate) struct UserShellResource {
    pub(crate) name: String,
    pub(crate) path: PathBuf,
}

#[derive(Debug, Clone)]
pub(crate) struct SystemGroupResource {
    pub(crate) name: String,
}

#[derive(Debug, Clone)]
pub(crate) struct UserGroupResource {
    pub(crate) name: String,
}

pub(crate) fn resolve_shell(name: &str) -> Result<UserShellResource> {
    let path = command_path(name).with_context(|| format!("could not find shell: {name}"))?;
    Ok(UserShellResource {
        name: name.to_string(),
        path,
    })
}

fn command_path(name: &str) -> Option<PathBuf> {
    if name.contains('/') {
        let path = PathBuf::from(name);
        return path.exists().then_some(path);
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|path| path.exists())
}

pub(crate) fn current_shell() -> Option<PathBuf> {
    let user = current_user()?;
    if std::env::consts::OS == "macos" {
        return macos_current_shell(&user).or_else(env_shell);
    }
    passwd_shell(&user).or_else(env_shell)
}

fn passwd_shell(user: &str) -> Option<PathBuf> {
    let passwd = fs::read_to_string("/etc/passwd").ok()?;
    passwd.lines().find_map(|line| {
        let mut parts = line.split(':');
        let name = parts.next()?;
        if name != user {
            return None;
        }
        parts.nth(5).map(PathBuf::from)
    })
}

fn macos_current_shell(user: &str) -> Option<PathBuf> {
    let output = ProcessCommand::new("dscl")
        .args([".", "-read", &format!("/Users/{user}"), "UserShell"])
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let output = String::from_utf8_lossy(&output.stdout);
    output
        .split_whitespace()
        .last()
        .filter(|shell| shell.starts_with('/'))
        .map(PathBuf::from)
}

fn env_shell() -> Option<PathBuf> {
    std::env::var_os("SHELL").map(PathBuf::from)
}

pub(crate) fn shell_matches(resource: &UserShellResource) -> bool {
    current_shell()
        .map(|shell| shell == resource.path || shell_name_matches(&shell, &resource.name))
        .unwrap_or(false)
}

fn shell_name_matches(shell: &std::path::Path, requested: &str) -> bool {
    !requested.contains('/') && shell.file_name().and_then(|name| name.to_str()) == Some(requested)
}

pub(crate) fn apply_shell(resource: &UserShellResource) -> Result<()> {
    let status = ProcessCommand::new("chsh")
        .arg("-s")
        .arg(&resource.path)
        .stdin(Stdio::null())
        .status()
        .with_context(|| "failed to run chsh")?;
    if !status.success() {
        bail!("failed to set shell to {}", resource.path.display());
    }
    Ok(())
}

pub(crate) fn current_groups() -> Result<BTreeSet<String>> {
    let user = current_user().context("could not determine current user")?;
    let output = ProcessCommand::new("id")
        .arg("-nG")
        .arg(&user)
        .stdin(Stdio::null())
        .output()
        .with_context(|| "failed to run id")?;
    if !output.status.success() {
        bail!("failed to list groups for {user}");
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .map(str::to_string)
        .collect())
}

pub(crate) fn user_in_group(resource: &UserGroupResource) -> Result<bool> {
    Ok(current_groups()?.contains(&resource.name))
}

pub(crate) fn system_group_exists(name: &str) -> Result<bool> {
    let status = ProcessCommand::new("getent")
        .arg("group")
        .arg(name)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| "failed to run getent")?;
    Ok(status.success())
}

pub(crate) fn create_group(resource: &SystemGroupResource) -> Result<()> {
    if std::env::consts::OS != "linux" {
        bail!("groups are only supported on Linux");
    }
    let status = ProcessCommand::new("sudo")
        .arg("groupadd")
        .arg(&resource.name)
        .status()
        .with_context(|| "failed to run groupadd")?;
    if !status.success() {
        bail!("failed to create group {}", resource.name);
    }
    Ok(())
}

pub(crate) fn remove_group(resource: &SystemGroupResource) -> Result<()> {
    if std::env::consts::OS != "linux" {
        bail!("groups are only supported on Linux");
    }
    if !system_group_exists(&resource.name)? {
        return Ok(());
    }
    let status = ProcessCommand::new("sudo")
        .arg("groupdel")
        .arg(&resource.name)
        .status()
        .with_context(|| "failed to run groupdel")?;
    if !status.success() {
        bail!("failed to remove group {}", resource.name);
    }
    Ok(())
}

pub(crate) fn apply_group(resource: &UserGroupResource) -> Result<()> {
    if std::env::consts::OS != "linux" {
        bail!("user groups are only supported on Linux");
    }
    let user = current_user().context("could not determine current user")?;
    let status = ProcessCommand::new("sudo")
        .arg("usermod")
        .arg("-aG")
        .arg(&resource.name)
        .arg(&user)
        .status()
        .with_context(|| "failed to run usermod")?;
    if !status.success() {
        bail!("failed to add {user} to group {}", resource.name);
    }
    Ok(())
}

pub(crate) fn remove_user_from_group(resource: &UserGroupResource) -> Result<()> {
    if std::env::consts::OS != "linux" {
        bail!("user groups are only supported on Linux");
    }
    if !user_in_group(resource)? {
        return Ok(());
    }
    let user = current_user().context("could not determine current user")?;
    let status = ProcessCommand::new("sudo")
        .arg("gpasswd")
        .arg("-d")
        .arg(&user)
        .arg(&resource.name)
        .status()
        .with_context(|| "failed to run gpasswd")?;
    if !status.success() {
        bail!("failed to remove {user} from group {}", resource.name);
    }
    Ok(())
}

fn current_user() -> Option<String> {
    std::env::var("USER").ok().filter(|user| !user.is_empty())
}
