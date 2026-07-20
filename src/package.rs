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
    BrewTrustedFormulae,
    BrewTrustedTaps,
}

impl PackageListFormat {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Lines => "lines",
            Self::BrewFormulae => "brew-formulae",
            Self::BrewCasks => "brew-casks",
            Self::BrewTrustedFormulae => "brew-trusted-formulae",
            Self::BrewTrustedTaps => "brew-trusted-taps",
        }
    }

    pub(crate) fn from_str(value: &str) -> Option<Self> {
        match value {
            "lines" => Some(Self::Lines),
            "brew-formulae" => Some(Self::BrewFormulae),
            "brew-casks" => Some(Self::BrewCasks),
            "brew-trusted-formulae" => Some(Self::BrewTrustedFormulae),
            "brew-trusted-taps" => Some(Self::BrewTrustedTaps),
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
type OptionalCommandOutput = Option<Vec<u8>>;

#[derive(Debug, Default)]
pub(crate) struct PackageStatusCache {
    providers: BTreeMap<String, OptionalPackageSet>,
    command_outputs: BTreeMap<String, OptionalCommandOutput>,
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
    let packages = list_installed_packages(cache, provider)?;
    cache.providers.insert(provider_name.to_string(), packages);
    Ok(())
}

fn list_installed_packages(
    cache: &mut PackageStatusCache,
    provider: &PackageProvider,
) -> Result<Option<BTreeSet<String>>> {
    let Some(list) = &provider.list else {
        return Ok(None);
    };
    match list.format {
        PackageListFormat::Lines => command_output_set_cached(cache, &list.command, parse_lines),
        PackageListFormat::BrewFormulae => {
            command_output_set_cached(cache, &list.command, |source| {
                parse_brew_info_json(source).map(|(formulae, _)| formulae)
            })
        }
        PackageListFormat::BrewCasks => command_output_set_cached(cache, &list.command, |source| {
            parse_brew_info_json(source).map(|(_, casks)| casks)
        }),
        PackageListFormat::BrewTrustedFormulae => {
            command_output_set_cached(cache, &list.command, |source| {
                parse_json_string_array(source, "formulae")
            })
        }
        PackageListFormat::BrewTrustedTaps => {
            command_output_set_cached(cache, &list.command, |source| {
                parse_json_string_array(source, "taps")
            })
        }
    }
}

fn parse_json_string_array(source: &[u8], key: &str) -> Result<BTreeSet<String>> {
    let value: serde_json::Value = serde_json::from_slice(source)?;
    let mut values = BTreeSet::new();
    insert_json_string_array(&mut values, value.get(key));
    Ok(values)
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

fn parse_lines(source: &[u8]) -> Result<BTreeSet<String>> {
    Ok(String::from_utf8_lossy(source)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect())
}

fn command_output_set_cached(
    cache: &mut PackageStatusCache,
    command: &str,
    parse: impl FnOnce(&[u8]) -> Result<BTreeSet<String>>,
) -> Result<Option<BTreeSet<String>>> {
    if !cache.command_outputs.contains_key(command) {
        let output = command_output(command)?;
        cache.command_outputs.insert(command.to_string(), output);
    }
    let Some(output) = cache
        .command_outputs
        .get(command)
        .and_then(|output| output.as_ref())
    else {
        return Ok(None);
    };
    Ok(Some(parse(output)?))
}

fn command_output(command: &str) -> Result<Option<Vec<u8>>> {
    let output = ProcessCommand::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(Some(output.stdout))
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
    use std::fs;

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
    fn brew_trust_parser_includes_requested_entries() {
        let source = br#"
        {
          "taps": ["example/tools"],
          "formulae": ["example/tools/widget"],
          "casks": [],
          "commands": []
        }
        "#;

        let taps = parse_json_string_array(source, "taps").unwrap();
        let formulae = parse_json_string_array(source, "formulae").unwrap();

        assert!(taps.contains("example/tools"));
        assert!(formulae.contains("example/tools/widget"));
    }

    #[test]
    fn shared_list_command_runs_once() {
        let root = tempfile::tempdir().unwrap();
        let count = root.path().join("count");
        let command = format!(
            "n=$(cat '{}' 2>/dev/null || echo 0); echo $((n + 1)) > '{}'; printf '%s\\n' bat",
            count.display(),
            count.display()
        );
        let provider = PackageProvider {
            capability: "provider:fake".to_string(),
            available: "exit 0".to_string(),
            installed: "exit 1".to_string(),
            install: "exit 0".to_string(),
            remove: "exit 0".to_string(),
            list: Some(PackageList {
                command,
                format: PackageListFormat::Lines,
            }),
            package_provides: BTreeMap::new(),
            matcher: PackageMatcher::Exact,
        };
        let mut cache = PackageStatusCache::default();

        assert!(
            package_installed_cached(
                &mut cache,
                &provider,
                &PackageResource {
                    provider: "one".to_string(),
                    name: "bat".to_string(),
                },
            )
            .unwrap()
        );
        assert!(
            package_installed_cached(
                &mut cache,
                &provider,
                &PackageResource {
                    provider: "two".to_string(),
                    name: "bat".to_string(),
                },
            )
            .unwrap()
        );

        assert_eq!(fs::read_to_string(count).unwrap().trim(), "1");
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
