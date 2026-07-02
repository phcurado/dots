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
    pub(crate) capability: String,
    pub(crate) available: String,
    pub(crate) installed: String,
    pub(crate) install: String,
    pub(crate) remove: String,
    pub(crate) list: Option<PackageList>,
    pub(crate) package_provides: BTreeMap<String, String>,
    pub(crate) matcher: PackageMatcher,
}

impl PackageProvider {
    pub(crate) fn capability_name(&self) -> String {
        self.capability
            .strip_prefix("provider:")
            .unwrap_or(&self.capability)
            .to_string()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PackageList {
    pub(crate) command: String,
    pub(crate) format: PackageListFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PackageListFormat {
    Lines,
    BrewFormulae,
    BrewCasks,
}

impl PackageListFormat {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Lines => "lines",
            Self::BrewFormulae => "brew-formulae",
            Self::BrewCasks => "brew-casks",
        }
    }

    pub(crate) fn from_str(value: &str) -> Option<Self> {
        match value {
            "lines" => Some(Self::Lines),
            "brew-formulae" => Some(Self::BrewFormulae),
            "brew-casks" => Some(Self::BrewCasks),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PackageMatcher {
    Exact,
    Basename,
    CaseInsensitive,
}

impl PackageMatcher {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Basename => "basename",
            Self::CaseInsensitive => "case-insensitive",
        }
    }

    pub(crate) fn from_str(value: &str) -> Option<Self> {
        match value {
            "exact" => Some(Self::Exact),
            "basename" => Some(Self::Basename),
            "case-insensitive" => Some(Self::CaseInsensitive),
            _ => None,
        }
    }
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

pub(crate) fn package_provides(
    provider: &PackageProvider,
    resource: &PackageResource,
) -> Vec<String> {
    provider
        .package_provides
        .get(&resource.name)
        .cloned()
        .into_iter()
        .collect()
}

pub(crate) fn package_installed_cached(
    cache: &mut PackageStatusCache,
    provider: &PackageProvider,
    resource: &PackageResource,
) -> Result<bool> {
    if let Some(packages) = packages_for_provider(cache, &resource.provider, provider)? {
        return Ok(package_set_contains(
            &packages,
            provider.matcher,
            &resource.name,
        ));
    }
    package_installed(provider, resource)
}

fn packages_for_provider(
    cache: &mut PackageStatusCache,
    provider_name: &str,
    provider: &PackageProvider,
) -> Result<Option<BTreeSet<String>>> {
    if !cache.providers.contains_key(provider_name) {
        populate_provider_cache(cache, provider_name, provider)?;
    }
    Ok(cache.providers.get(provider_name).cloned().unwrap_or(None))
}

fn populate_provider_cache(
    cache: &mut PackageStatusCache,
    provider_name: &str,
    provider: &PackageProvider,
) -> Result<()> {
    let packages = list_installed_packages(provider)?;
    cache.providers.insert(provider_name.to_string(), packages);
    Ok(())
}

fn list_installed_packages(provider: &PackageProvider) -> Result<Option<BTreeSet<String>>> {
    let Some(list) = &provider.list else {
        return Ok(None);
    };
    match list.format {
        PackageListFormat::Lines => command_set(&list.command),
        PackageListFormat::BrewFormulae => command_output_set(&list.command, |source| {
            parse_brew_info_json(source).map(|(formulae, _)| formulae)
        }),
        PackageListFormat::BrewCasks => command_output_set(&list.command, |source| {
            parse_brew_info_json(source).map(|(_, casks)| casks)
        }),
    }
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
    command_output_set(command, |source| {
        Ok(String::from_utf8_lossy(source)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect())
    })
}

fn command_output_set(
    command: &str,
    parse: impl FnOnce(&[u8]) -> Result<BTreeSet<String>>,
) -> Result<Option<BTreeSet<String>>> {
    let output = ProcessCommand::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(Some(parse(&output.stdout)?))
}

fn package_set_contains(packages: &BTreeSet<String>, matcher: PackageMatcher, name: &str) -> bool {
    match matcher {
        PackageMatcher::Exact => packages.contains(name),
        PackageMatcher::Basename => {
            packages.contains(name) || packages.contains(name.rsplit('/').next().unwrap_or(name))
        }
        PackageMatcher::CaseInsensitive => packages
            .iter()
            .any(|package| package.eq_ignore_ascii_case(name)),
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
    fn basename_matcher_accepts_last_path_component() {
        let packages = BTreeSet::from(["aerospace".to_string()]);

        assert!(package_set_contains(
            &packages,
            PackageMatcher::Basename,
            "nikitabobko/tap/aerospace"
        ));
    }

    #[test]
    fn case_insensitive_matcher_accepts_different_case() {
        let packages = BTreeSet::from(["felixkratz/formulae".to_string()]);

        assert!(package_set_contains(
            &packages,
            PackageMatcher::CaseInsensitive,
            "FelixKratz/formulae"
        ));
    }
}
