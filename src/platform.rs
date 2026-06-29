use std::collections::BTreeMap;
use std::fs;

use anyhow::{Context, Result};
use mlua::{Lua, Table};

pub(crate) fn selected_profile(profile: Option<&str>) -> Result<String> {
    if let Some(profile) = profile {
        return Ok(profile.to_string());
    }
    if let Ok(profile) = std::env::var("DOTS_PROFILE") {
        if !profile.is_empty() {
            return Ok(profile);
        }
    }
    hostname()
        .or_else(|| std::env::var("USER").ok())
        .context("could not determine default profile")
}

pub(crate) fn hostname() -> Option<String> {
    if let Ok(hostname) = std::env::var("HOSTNAME") {
        if !hostname.is_empty() {
            return Some(hostname);
        }
    }
    fs::read_to_string("/etc/hostname")
        .ok()
        .map(|hostname| hostname.trim().to_string())
        .filter(|hostname| !hostname.is_empty())
}

pub(crate) fn platform_table(lua: &Lua) -> Result<Table> {
    let table = lua.create_table()?;
    let os = platform_os();
    let os_release = if os == "linux" {
        linux_os_release().unwrap_or_default()
    } else {
        BTreeMap::new()
    };
    let distro = os_release.get("ID").cloned();
    let family = platform_family(os, &os_release);

    table.set("system", format!("{}-{os}", std::env::consts::ARCH))?;
    table.set("arch", std::env::consts::ARCH)?;
    table.set("os", os)?;
    table.set("hostname", hostname().unwrap_or_default())?;
    table.set("distro", distro)?;
    table.set("family", family)?;
    Ok(table)
}

fn platform_os() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        os => os,
    }
}

fn platform_family(os: &str, os_release: &BTreeMap<String, String>) -> String {
    match os {
        "darwin" => "darwin".to_string(),
        "linux" => linux_family(os_release),
        os => os.to_string(),
    }
}

fn linux_family(os_release: &BTreeMap<String, String>) -> String {
    let id = os_release.get("ID").map(String::as_str).unwrap_or_default();
    let id_like = os_release
        .get("ID_LIKE")
        .map(String::as_str)
        .unwrap_or_default();
    if id == "arch" || id_like.split_whitespace().any(|value| value == "arch") {
        "arch".to_string()
    } else if id == "debian"
        || id == "ubuntu"
        || id_like
            .split_whitespace()
            .any(|value| value == "debian" || value == "ubuntu")
    {
        "debian".to_string()
    } else {
        id.to_string()
    }
}

fn linux_os_release() -> Option<BTreeMap<String, String>> {
    let source = fs::read_to_string("/etc/os-release").ok()?;
    Some(parse_os_release(&source))
}

fn parse_os_release(source: &str) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        values.insert(key.to_string(), unquote_os_release_value(value));
    }
    values
}

fn unquote_os_release_value(value: &str) -> String {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os_release(values: &[(&str, &str)]) -> BTreeMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    #[test]
    fn detects_linux_package_family() {
        assert_eq!(
            platform_family("linux", &os_release(&[("ID", "arch")])),
            "arch"
        );
        assert_eq!(
            platform_family(
                "linux",
                &os_release(&[("ID", "ubuntu"), ("ID_LIKE", "debian")])
            ),
            "debian"
        );
        assert_eq!(
            platform_family(
                "linux",
                &os_release(&[("ID", "pop"), ("ID_LIKE", "ubuntu debian")])
            ),
            "debian"
        );
    }

    #[test]
    fn darwin_family_is_darwin() {
        assert_eq!(platform_family("darwin", &BTreeMap::new()), "darwin");
    }

    #[test]
    fn parses_os_release() {
        let values = parse_os_release(
            r#"
            ID=ubuntu
            ID_LIKE="debian"
            "#,
        );

        assert_eq!(values.get("ID").unwrap(), "ubuntu");
        assert_eq!(values.get("ID_LIKE").unwrap(), "debian");
    }

    #[test]
    fn explicit_profile_wins() {
        assert_eq!(selected_profile(Some("work")).unwrap(), "work");
    }
}
