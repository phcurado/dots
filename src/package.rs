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

type PackageSet = BTreeSet<String>;
type OptionalPackageSet = Option<PackageSet>;

#[derive(Debug, Default)]
pub(crate) struct PackageStatusCache {
    providers: BTreeMap<String, OptionalPackageSet>,
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
    if !cache.providers.contains_key(provider) {
        populate_provider_cache(cache, provider)?;
    }
    Ok(cache.providers.get(provider).cloned().unwrap_or(None))
}

fn populate_provider_cache(cache: &mut PackageStatusCache, provider: &str) -> Result<()> {
    if matches!(provider, "brew" | "brew-cask") {
        let (formulae, casks) = list_brew_packages()?;
        cache.providers.insert("brew".to_string(), formulae);
        cache.providers.insert("brew-cask".to_string(), casks);
        return Ok(());
    }

    let packages = list_installed_packages(provider)?;
    cache.providers.insert(provider.to_string(), packages);
    Ok(())
}

fn list_installed_packages(provider: &str) -> Result<Option<BTreeSet<String>>> {
    let command = match provider {
        "pacman" | "paru" => "pacman -Qq",
        "apt" => "dpkg-query -W -f='${binary:Package}\\n'",
        "brew-tap" => "brew tap",
        _ => return Ok(None),
    };
    command_set(command)
}

fn list_brew_packages() -> Result<(OptionalPackageSet, OptionalPackageSet)> {
    let output = ProcessCommand::new("brew")
        .args(["info", "--json=v2", "--installed"])
        .stdin(Stdio::null())
        .output()?;
    if output.status.success() {
        return parse_brew_info_json(&output.stdout)
            .map(|(formulae, casks)| (Some(formulae), Some(casks)));
    }

    Ok((
        command_set("brew list --formula -1")?,
        command_set("brew list --cask -1")?,
    ))
}

fn parse_brew_info_json(source: &[u8]) -> Result<(BTreeSet<String>, BTreeSet<String>)> {
    let value: serde_json::Value = serde_json::from_slice(source)?;
    let mut formulae = BTreeSet::new();
    for formula in value
        .get("formulae")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
    {
        insert_json_string(&mut formulae, formula.get("name"));
        insert_json_string(&mut formulae, formula.get("full_name"));
        insert_json_string_array(&mut formulae, formula.get("aliases"));
        insert_json_string_array(&mut formulae, formula.get("oldnames"));
    }

    let mut casks = BTreeSet::new();
    for cask in value
        .get("casks")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
    {
        insert_json_string(&mut casks, cask.get("token"));
        insert_json_string(&mut casks, cask.get("full_token"));
        insert_json_string_array(&mut casks, cask.get("old_tokens"));
    }

    Ok((formulae, casks))
}

fn insert_json_string(values: &mut BTreeSet<String>, value: Option<&serde_json::Value>) {
    if let Some(value) = value.and_then(|value| value.as_str()) {
        values.insert(value.to_string());
    }
}

fn insert_json_string_array(values: &mut BTreeSet<String>, value: Option<&serde_json::Value>) {
    let Some(values_json) = value.and_then(|value| value.as_array()) else {
        return;
    };
    for value in values_json {
        insert_json_string(values, Some(value));
    }
}

fn command_set(command: &str) -> Result<Option<BTreeSet<String>>> {
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
    match provider {
        "brew" | "brew-cask" => {
            packages.contains(name) || packages.contains(name.rsplit('/').next().unwrap_or(name))
        }
        "brew-tap" => packages
            .iter()
            .any(|package| package.eq_ignore_ascii_case(name)),
        _ => packages.contains(name),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brew_info_parser_includes_formula_aliases() {
        let source = br#"
        {
          "formulae": [
            {
              "name": "sevenzip",
              "full_name": "sevenzip",
              "aliases": ["7zip"],
              "oldnames": []
            }
          ],
          "casks": [
            {
              "token": "aerospace",
              "full_token": "nikitabobko/tap/aerospace",
              "old_tokens": []
            }
          ]
        }
        "#;

        let (formulae, casks) = parse_brew_info_json(source).unwrap();

        assert!(formulae.contains("sevenzip"));
        assert!(formulae.contains("7zip"));
        assert!(casks.contains("aerospace"));
        assert!(casks.contains("nikitabobko/tap/aerospace"));
    }

    #[test]
    fn brew_taps_match_case_insensitively() {
        let packages = BTreeSet::from(["felixkratz/formulae".to_string()]);

        assert!(package_set_contains(
            &packages,
            "brew-tap",
            "FelixKratz/formulae"
        ));
    }
}
