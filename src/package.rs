use std::collections::{BTreeMap, BTreeSet};
use std::process::{Command as ProcessCommand, Stdio};

use anyhow::Result;

#[derive(Debug, Clone)]
pub(crate) struct PackageResource {
    pub(crate) provider: String,
    pub(crate) name: String,
}

#[derive(Debug, Clone)]
pub(crate) struct PackageProvider {
    pub(crate) available: String,
    pub(crate) installed: String,
    pub(crate) install: String,
    pub(crate) remove: String,
}

pub(crate) fn package_provider_available(provider: &PackageProvider) -> Result<bool> {
    run_provider_command(&provider.available, None, true)
}

pub(crate) fn package_installed(
    provider: &PackageProvider,
    resource: &PackageResource,
) -> Result<bool> {
    run_provider_command(&provider.installed, Some(&resource.name), true)
}

#[derive(Debug, Default)]
pub(crate) struct PackageStatusCache {
    providers: BTreeMap<String, Option<BTreeSet<String>>>,
}

pub(crate) fn package_installed_cached(
    cache: &mut PackageStatusCache,
    provider: &PackageProvider,
    resource: &PackageResource,
) -> Result<bool> {
    if let Some(packages) = packages_for_provider(cache, &resource.provider)? {
        return Ok(package_set_contains(
            &packages,
            &resource.provider,
            &resource.name,
        ));
    }
    package_installed(provider, resource)
}

fn packages_for_provider(
    cache: &mut PackageStatusCache,
    provider: &str,
) -> Result<Option<BTreeSet<String>>> {
    if let Some(packages) = cache.providers.get(provider) {
        return Ok(packages.clone());
    }
    let packages = list_installed_packages(provider)?;
    cache
        .providers
        .insert(provider.to_string(), packages.clone());
    Ok(packages)
}

fn list_installed_packages(provider: &str) -> Result<Option<BTreeSet<String>>> {
    let command = match provider {
        "pacman" | "paru" => "pacman -Qq",
        "apt" => "dpkg-query -W -f='${binary:Package}\\n'",
        "brew" => "brew list --formula -1",
        "brew-cask" => "brew list --cask -1",
        "brew-tap" => "brew tap",
        _ => return Ok(None),
    };
    let output = ProcessCommand::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(Some(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect(),
    ))
}

fn package_set_contains(packages: &BTreeSet<String>, provider: &str, name: &str) -> bool {
    packages.contains(name)
        || matches!(provider, "brew" | "brew-cask")
            && packages.contains(name.rsplit('/').next().unwrap_or(name))
}

pub(crate) fn run_provider_command(
    command: &str,
    package: Option<&str>,
    quiet: bool,
) -> Result<bool> {
    let mut process = ProcessCommand::new("sh");
    process.arg("-c").arg(command);
    process.stdin(Stdio::null());
    if let Some(package) = package {
        process.env("DOTS_PACKAGE", package);
    }
    if quiet {
        process.stdout(Stdio::null()).stderr(Stdio::null());
    }
    Ok(process.status()?.success())
}
