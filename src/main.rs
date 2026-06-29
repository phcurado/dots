use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::IsTerminal;
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use globset::{Glob, GlobSet, GlobSetBuilder};
use mlua::{Lua, Table, Value};
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
#[command(name = "dots", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Lua config file. Defaults to dots.lua in the project root.
    #[arg(short, long, global = true)]
    file: Option<PathBuf>,

    /// Profile name for host or machine-specific config.
    #[arg(long, global = true)]
    profile: Option<String>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show the planned changes without applying them.
    Plan,
    /// Apply the planned changes.
    Apply,
    /// Inspect or edit local state.
    State {
        #[command(subcommand)]
        command: StateCommand,
    },
}

#[derive(Debug, Subcommand)]
enum StateCommand {
    /// List resources tracked in state.
    List,
    /// Stop tracking a resource without changing the filesystem.
    Forget { resource: String },
}

#[derive(Debug, Default)]
struct Config {
    symlinks: Vec<SymlinkResource>,
    packages: Vec<PackageResource>,
    package_providers: BTreeMap<String, PackageProvider>,
}

#[derive(Debug, Clone)]
struct SymlinkResource {
    target: PathBuf,
    source: PathBuf,
}

#[derive(Debug, Clone)]
struct SymlinkDeclaration {
    target: PathBuf,
    source: PathBuf,
    ignore: Vec<String>,
}

#[derive(Debug, Clone)]
struct PackageResource {
    provider: String,
    name: String,
}

#[derive(Debug, Clone)]
struct PackageProvider {
    available: String,
    installed: String,
    install: String,
    remove: String,
}

#[derive(Debug, Clone)]
enum PlanStep {
    SymlinkCreate(SymlinkResource),
    SymlinkUpdate(SymlinkResource),
    SymlinkRemove {
        target: PathBuf,
        source: PathBuf,
    },
    SymlinkNoop(SymlinkResource),
    SymlinkConflict {
        resource: SymlinkResource,
        reason: String,
    },
    PackageCreate {
        resource: PackageResource,
        provider: PackageProvider,
    },
    PackageRemove {
        resource: PackageResource,
        provider: PackageProvider,
    },
    PackageNoop(PackageResource),
    PackageConflict {
        resource: PackageResource,
        reason: String,
    },
}

#[derive(Debug, Default)]
struct PlanSummary {
    create: usize,
    update: usize,
    remove: usize,
    conflicts: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
enum StateResource {
    #[serde(rename = "symlink")]
    Symlink { target: PathBuf, source: PathBuf },
    #[serde(rename = "package")]
    Package { provider: String, name: String },
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct State {
    resources: BTreeMap<String, StateResource>,
}

#[derive(Debug)]
struct Project {
    root: PathBuf,
    config: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let profile = selected_profile(cli.profile.as_deref())?;
    let project = find_project(cli.file)?;
    let state_path = project.root.join(".dots/state.json");
    let state_exists = state_path.exists();
    let mut state = load_state(&state_path)?;

    match cli.command {
        Command::Plan => {
            let config = load_config(&project, &profile)?;
            if !state_exists {
                print_state_initialized(&project, &state_path);
            }
            refresh_state_from_system(&config, &mut state)?;
            save_state(&state_path, &state)?;
            let plan = build_plan(&config, &state)?;
            print_plan(&project, &plan);
        }
        Command::Apply => {
            let config = load_config(&project, &profile)?;
            if !state_exists {
                print_state_initialized(&project, &state_path);
            }
            refresh_state_from_system(&config, &mut state)?;
            save_state(&state_path, &state)?;
            let plan = build_plan(&config, &state)?;
            print_plan(&project, &plan);
            apply_plan(&plan, &mut state)?;
            save_state(&state_path, &state)?;
        }
        Command::State { command } => {
            run_state_command(&project, &state_path, &mut state, command)?;
        }
    }

    Ok(())
}

fn selected_profile(profile: Option<&str>) -> Result<String> {
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

fn hostname() -> Option<String> {
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

fn platform_table(lua: &Lua) -> Result<Table> {
    let table = lua.create_table()?;
    let os = platform_os();
    let distro = linux_os_release().and_then(|values| values.get("ID").cloned());
    let family = platform_family(distro.as_deref());

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

fn platform_family(distro: Option<&str>) -> String {
    match std::env::consts::OS {
        "macos" => "darwin".to_string(),
        "linux" => {
            let values = linux_os_release().unwrap_or_default();
            let id = distro.unwrap_or_default();
            let id_like = values
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
        os => os.to_string(),
    }
}

fn linux_os_release() -> Option<BTreeMap<String, String>> {
    let source = fs::read_to_string("/etc/os-release").ok()?;
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
    Some(values)
}

fn unquote_os_release_value(value: &str) -> String {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
        .to_string()
}

fn find_project(file: Option<PathBuf>) -> Result<Project> {
    if let Some(config) = file {
        let config = fs::canonicalize(&config)
            .with_context(|| format!("failed to resolve {}", config.display()))?;
        let root = config
            .parent()
            .context("config path has no parent")?
            .to_path_buf();
        return Ok(Project { root, config });
    }

    let mut dir = std::env::current_dir()?;
    loop {
        let config = dir.join("dots.lua");
        if config.exists() {
            return Ok(Project { root: dir, config });
        }

        if !dir.pop() {
            bail!("could not find dots.lua")
        }
    }
}

fn load_config(project: &Project, profile: &str) -> Result<Config> {
    let lua = Lua::new();
    let symlinks = lua.create_table()?;
    let packages = lua.create_table()?;
    let package_providers = lua.create_table()?;
    let dots = lua.create_table()?;

    let root = project.root.clone();
    let collected_symlinks = symlinks.clone();
    let symlink = lua.create_function(move |lua, args: mlua::MultiValue| {
        let values = args.into_iter().collect::<Vec<_>>();
        if values.len() != 2 && values.len() != 3 {
            return Err(mlua::Error::RuntimeError(
                "expected dots.symlink(target, source[, opts])".to_string(),
            ));
        }

        let target = value_to_string(&values[0], "target")?;
        let source = value_to_string(&values[1], "source")?;

        let item = lua.create_table()?;
        item.set("target", expand_home(&target).display().to_string())?;
        item.set(
            "source",
            resolve_source(&root, &source).display().to_string(),
        )?;

        if let Some(opts) = values.get(2) {
            let Value::Table(opts) = opts else {
                return Err(mlua::Error::RuntimeError(
                    "dots.symlink opts must be a table".to_string(),
                ));
            };
            if let Some(ignore) = opts.get::<Option<Table>>("ignore")? {
                item.set("ignore", ignore)?;
            }
        }

        collected_symlinks.raw_push(item)?;
        Ok(())
    })?;

    let platform = platform_table(&lua)?;

    dots.set("symlink", symlink)?;
    dots.set("profile", profile)?;
    dots.set("platform", platform)?;
    dots.set("root", project.root.display().to_string())?;
    dots.set("os", std::env::consts::OS)?;

    install_provider_api(&lua, &dots, package_providers.clone(), packages.clone())?;

    let package: Table = lua.globals().get("package")?;
    let preload: Table = package.get("preload")?;
    let dots_module = dots.clone();
    preload.set(
        "dots",
        lua.create_function(move |_, ()| Ok(dots_module.clone()))?,
    )?;
    lua.globals().set("dots", dots)?;

    install_package_path(&lua, &project.root)?;
    load_builtin_lua(&lua)?;

    let source = fs::read_to_string(&project.config)
        .with_context(|| format!("failed to read {}", project.config.display()))?;
    lua.load(&source)
        .set_name(project.config.display().to_string())
        .exec()?;

    let mut config = Config::default();
    for item in symlinks.sequence_values::<Table>() {
        let item = item?;
        let declaration = SymlinkDeclaration {
            target: PathBuf::from(item.get::<String>("target")?),
            source: PathBuf::from(item.get::<String>("source")?),
            ignore: table_strings(item.get::<Option<Table>>("ignore")?)?,
        };
        config
            .symlinks
            .extend(expand_symlink_declaration(&declaration)?);
    }
    for item in packages.sequence_values::<Table>() {
        let item = item?;
        config.packages.push(PackageResource {
            provider: item.get::<String>("provider")?,
            name: item.get::<String>("name")?,
        });
    }
    for pair in package_providers.pairs::<String, Table>() {
        let (name, item) = pair?;
        config.package_providers.insert(
            name,
            PackageProvider {
                available: item.get::<String>("available")?,
                installed: item.get::<String>("installed")?,
                install: item.get::<String>("install")?,
                remove: item.get::<String>("remove")?,
            },
        );
    }
    dedupe_config(&mut config)?;
    Ok(config)
}

fn dedupe_config(config: &mut Config) -> Result<()> {
    let mut symlinks = BTreeMap::<PathBuf, SymlinkResource>::new();
    for resource in config.symlinks.drain(..) {
        match symlinks.get(&resource.target) {
            Some(existing) if same_path(&existing.source, &resource.source) => {}
            Some(existing) => bail!(
                "duplicate symlink target {} points to both {} and {}",
                resource.target.display(),
                existing.source.display(),
                resource.source.display()
            ),
            None => {
                symlinks.insert(resource.target.clone(), resource);
            }
        }
    }
    config.symlinks = symlinks.into_values().collect();

    let mut packages = BTreeMap::<String, PackageResource>::new();
    for resource in config.packages.drain(..) {
        packages.insert(package_id_for(&resource), resource);
    }
    config.packages = packages.into_values().collect();

    Ok(())
}

fn install_provider_api(lua: &Lua, dots: &Table, providers: Table, packages: Table) -> Result<()> {
    let provider_api = lua.create_table()?;
    let dots_for_provider = dots.clone();
    let package = lua.create_function(move |lua, (name, spec): (String, Table)| {
        let provider = PackageProvider {
            available: spec.get("available")?,
            installed: spec.get("installed")?,
            install: spec.get("install")?,
            remove: spec.get("remove")?,
        };
        register_package_provider(
            lua,
            &dots_for_provider,
            &providers,
            packages.clone(),
            &name,
            provider,
        )
        .map_err(mlua::Error::external)
    })?;
    provider_api.set("package", package)?;
    dots.set("provider", provider_api)?;
    Ok(())
}

fn load_builtin_lua(lua: &Lua) -> Result<()> {
    lua.load(include_str!("lua/prelude.lua"))
        .set_name("dots builtin prelude")
        .exec()?;
    Ok(())
}

fn register_package_provider(
    lua: &Lua,
    dots: &Table,
    providers: &Table,
    packages: Table,
    name: &str,
    provider: PackageProvider,
) -> Result<()> {
    let spec = lua.create_table()?;
    spec.set("available", provider.available)?;
    spec.set("installed", provider.installed)?;
    spec.set("install", provider.install)?;
    spec.set("remove", provider.remove)?;
    providers.set(name, spec)?;
    dots.set(
        name,
        package_provider_table(lua, packages, name.to_string())?,
    )?;
    Ok(())
}

fn package_provider_table(lua: &Lua, packages: Table, provider: String) -> Result<Table> {
    let table = lua.create_table()?;
    let install = lua.create_function(move |lua, package_list: Table| {
        for name in table_strings(Some(package_list))? {
            let item = lua.create_table()?;
            item.set("provider", provider.clone())?;
            item.set("name", name)?;
            packages.raw_push(item)?;
        }
        Ok(())
    })?;
    table.set("install", install)?;
    Ok(table)
}

fn install_package_path(lua: &Lua, root: &Path) -> Result<()> {
    let package: Table = lua.globals().get("package")?;
    let path: String = package.get("path")?;
    let root = root.display();
    package.set(
        "path",
        format!("{root}/?.lua;{root}/?/init.lua;{root}/dots/?.lua;{root}/dots/?/init.lua;{path}"),
    )?;
    Ok(())
}

fn value_to_string(value: &Value, name: &str) -> mlua::Result<String> {
    match value {
        Value::String(value) => Ok(value.to_string_lossy()),
        other => Err(mlua::Error::RuntimeError(format!(
            "dots.symlink {name} must be a string, got {other:?}"
        ))),
    }
}

fn table_strings(table: Option<Table>) -> mlua::Result<Vec<String>> {
    let Some(table) = table else {
        return Ok(Vec::new());
    };
    let mut values = Vec::new();
    for value in table.sequence_values::<Value>() {
        match value? {
            Value::String(value) => values.push(value.to_string_lossy()),
            other => {
                return Err(mlua::Error::RuntimeError(format!(
                    "ignore patterns must be strings, got {other:?}"
                )));
            }
        }
    }
    Ok(values)
}

fn expand_home(path: &str) -> PathBuf {
    if path == "~" {
        return home_dir();
    }
    if let Some(rest) = path.strip_prefix("~/") {
        return home_dir().join(rest);
    }
    PathBuf::from(path)
}

fn resolve_source(root: &Path, source: &str) -> PathBuf {
    let path = expand_home(source);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn expand_symlink_declaration(declaration: &SymlinkDeclaration) -> Result<Vec<SymlinkResource>> {
    let target_is_directory = fs::symlink_metadata(&declaration.target)
        .map(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
        .unwrap_or(false);
    let should_expand =
        declaration.source.is_dir() && (!declaration.ignore.is_empty() || target_is_directory);

    if !should_expand {
        return Ok(vec![SymlinkResource {
            target: declaration.target.clone(),
            source: declaration.source.clone(),
        }]);
    }

    let ignore = build_ignore_set(&declaration.ignore)?;
    let mut resources = Vec::new();
    expand_symlink_dir(
        &declaration.target,
        &declaration.source,
        Path::new(""),
        &ignore,
        &mut resources,
    )?;
    Ok(resources)
}

fn expand_symlink_dir(
    target_root: &Path,
    source_root: &Path,
    relative: &Path,
    ignore: &GlobSet,
    resources: &mut Vec<SymlinkResource>,
) -> Result<()> {
    let source_dir = source_root.join(relative);
    for entry in fs::read_dir(&source_dir)
        .with_context(|| format!("failed to read {}", source_dir.display()))?
    {
        let entry = entry?;
        let name = entry.file_name();
        let relative = relative.join(name);
        if ignore.is_match(&relative) {
            continue;
        }

        let source = source_root.join(&relative);
        let target = target_root.join(&relative);
        let metadata = fs::symlink_metadata(&source)?;

        if metadata.is_dir()
            && fs::symlink_metadata(&target)
                .map(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
                .unwrap_or(false)
        {
            expand_symlink_dir(target_root, source_root, &relative, ignore, resources)?;
        } else {
            resources.push(SymlinkResource { target, source });
        }
    }
    Ok(())
}

fn build_ignore_set(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder
            .add(Glob::new(pattern).with_context(|| format!("invalid ignore pattern: {pattern}"))?);
        if let Some(dir) = pattern.strip_suffix("/**") {
            builder.add(Glob::new(dir).with_context(|| format!("invalid ignore pattern: {dir}"))?);
        }
    }
    Ok(builder.build()?)
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("~"))
}

fn load_state(path: &Path) -> Result<State> {
    let Some(source) = fs::read_to_string(path).ok() else {
        return Ok(State::default());
    };
    Ok(serde_json::from_str(&source)?)
}

fn save_state(path: &Path, state: &State) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(state)?)?;
    fs::rename(tmp, path)?;
    Ok(())
}

fn run_state_command(
    project: &Project,
    state_path: &Path,
    state: &mut State,
    command: StateCommand,
) -> Result<()> {
    match command {
        StateCommand::List => print_state(project, state),
        StateCommand::Forget { resource } => {
            let key = state_key_from_arg(&resource);
            if state.resources.remove(&key).is_some() {
                save_state(state_path, state)?;
                println!("{} {}", "Forgot:".bold(), resource);
            } else {
                bail!("resource is not tracked: {resource}");
            }
        }
    }
    Ok(())
}

fn print_state(project: &Project, state: &State) {
    if state.resources.is_empty() {
        println!("{}", dim("State is empty."));
        return;
    }

    println!("{}", bold("State:"));
    for (id, resource) in &state.resources {
        match resource {
            StateResource::Symlink { target, source } => println!(
                "  symlink {} -> {}",
                display_target(target),
                display_source(project, source),
            ),
            StateResource::Package { provider, name } => {
                println!("  package {provider} {name}")
            }
        }
        println!("    {}", dim(id));
    }
}

fn state_key_from_arg(resource: &str) -> String {
    if resource.starts_with("symlink:") || resource.starts_with("package:") {
        resource.to_string()
    } else {
        format!("symlink:{}", expand_home(resource).display())
    }
}

fn refresh_state_from_system(config: &Config, state: &mut State) -> Result<()> {
    for resource in &config.symlinks {
        if symlink_matches(resource)? {
            state
                .resources
                .insert(symlink_id_for(resource), state_symlink(resource));
        }
    }

    Ok(())
}

fn build_plan(config: &Config, state: &State) -> Result<Vec<PlanStep>> {
    let mut plan = Vec::new();
    let mut declared = BTreeSet::new();

    for resource in &config.symlinks {
        let id = symlink_id_for(resource);
        declared.insert(id.clone());
        let owned = state.resources.contains_key(&id);

        if !resource.source.exists() {
            plan.push(PlanStep::SymlinkConflict {
                resource: resource.clone(),
                reason: format!("source does not exist: {}", resource.source.display()),
            });
            continue;
        }

        match fs::symlink_metadata(&resource.target) {
            Ok(meta) if meta.file_type().is_symlink() => {
                let current = fs::read_link(&resource.target)?;
                let current = resolve_symlink_target(&resource.target, &current);
                if owned && same_path(&current, &resource.source) {
                    plan.push(PlanStep::SymlinkNoop(resource.clone()));
                } else if owned {
                    plan.push(PlanStep::SymlinkUpdate(resource.clone()));
                } else if same_path(&current, &resource.source) {
                    plan.push(PlanStep::SymlinkNoop(resource.clone()));
                } else {
                    plan.push(PlanStep::SymlinkConflict {
                        resource: resource.clone(),
                        reason: "target exists but is not managed".to_string(),
                    });
                }
            }
            Ok(_) => plan.push(PlanStep::SymlinkConflict {
                resource: resource.clone(),
                reason: "target exists and is not a symlink".to_string(),
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                plan.push(PlanStep::SymlinkCreate(resource.clone()));
            }
            Err(error) => return Err(error.into()),
        }
    }

    for resource in &config.packages {
        let id = package_id_for(resource);
        declared.insert(id.clone());

        let Some(provider) = config.package_providers.get(&resource.provider) else {
            plan.push(PlanStep::PackageConflict {
                resource: resource.clone(),
                reason: format!("{} provider is not configured", resource.provider),
            });
            continue;
        };
        if package_installed(provider, resource)? {
            plan.push(PlanStep::PackageNoop(resource.clone()));
        } else {
            plan.push(PlanStep::PackageCreate {
                resource: resource.clone(),
                provider: provider.clone(),
            });
        }
    }

    for (id, resource) in &state.resources {
        if declared.contains(id) {
            continue;
        }
        match resource {
            StateResource::Symlink { target, source } => plan.push(PlanStep::SymlinkRemove {
                target: target.clone(),
                source: source.clone(),
            }),
            StateResource::Package { provider, name } => {
                let resource = PackageResource {
                    provider: provider.clone(),
                    name: name.clone(),
                };
                match config.package_providers.get(provider) {
                    Some(provider) => plan.push(PlanStep::PackageRemove {
                        resource,
                        provider: provider.clone(),
                    }),
                    None => plan.push(PlanStep::PackageConflict {
                        resource,
                        reason: format!("{provider} provider is not configured"),
                    }),
                }
            }
        }
    }

    Ok(plan)
}

fn package_provider_available(provider: &PackageProvider) -> Result<bool> {
    run_provider_command(&provider.available, None, true)
}

fn package_installed(provider: &PackageProvider, resource: &PackageResource) -> Result<bool> {
    run_provider_command(&provider.installed, Some(&resource.name), true)
}

fn run_provider_command(command: &str, package: Option<&str>, quiet: bool) -> Result<bool> {
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

fn same_path(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn symlink_matches(resource: &SymlinkResource) -> Result<bool> {
    let Ok(meta) = fs::symlink_metadata(&resource.target) else {
        return Ok(false);
    };
    if !meta.file_type().is_symlink() || !resource.source.exists() {
        return Ok(false);
    }
    let current = fs::read_link(&resource.target)?;
    let current = resolve_symlink_target(&resource.target, &current);
    Ok(same_path(&current, &resource.source))
}

fn resolve_symlink_target(target: &Path, link: &Path) -> PathBuf {
    if link.is_absolute() {
        link.to_path_buf()
    } else {
        target.parent().unwrap_or_else(|| Path::new(".")).join(link)
    }
}

fn state_symlink(resource: &SymlinkResource) -> StateResource {
    StateResource::Symlink {
        target: resource.target.clone(),
        source: resource.source.clone(),
    }
}

fn state_package(resource: &PackageResource) -> StateResource {
    StateResource::Package {
        provider: resource.provider.clone(),
        name: resource.name.clone(),
    }
}

fn symlink_id_for(resource: &SymlinkResource) -> String {
    format!("symlink:{}", resource.target.display())
}

fn package_id_for(resource: &PackageResource) -> String {
    format!("package:{}:{}", resource.provider, resource.name)
}

fn display_target(path: &Path) -> String {
    let home = home_dir();
    if path == home {
        return "~".to_string();
    }
    if let Ok(rest) = path.strip_prefix(&home) {
        return format!("~/{}", rest.display());
    }
    path.display().to_string()
}

fn display_source(project: &Project, path: &Path) -> String {
    if let Ok(rest) = path.strip_prefix(&project.root) {
        if rest.as_os_str().is_empty() {
            return ".".to_string();
        }
        return rest.display().to_string();
    }
    display_target(path)
}

fn summarize_plan(plan: &[PlanStep]) -> PlanSummary {
    let mut summary = PlanSummary::default();
    for step in plan {
        match step {
            PlanStep::SymlinkCreate(_) | PlanStep::PackageCreate { .. } => summary.create += 1,
            PlanStep::SymlinkUpdate(_) => summary.update += 1,
            PlanStep::SymlinkRemove { .. } | PlanStep::PackageRemove { .. } => summary.remove += 1,
            PlanStep::SymlinkConflict { .. } | PlanStep::PackageConflict { .. } => {
                summary.conflicts += 1
            }
            PlanStep::SymlinkNoop(_) | PlanStep::PackageNoop(_) => {}
        }
    }
    summary
}

fn print_plan(project: &Project, plan: &[PlanStep]) {
    let summary = summarize_plan(plan);
    let has_changes = summary.create + summary.update + summary.remove + summary.conflicts > 0;
    if !has_changes {
        println!("{}", dim("No changes."));
        return;
    }

    let has_symlinks = plan.iter().any(|step| {
        matches!(
            step,
            PlanStep::SymlinkCreate(_)
                | PlanStep::SymlinkUpdate(_)
                | PlanStep::SymlinkRemove { .. }
                | PlanStep::SymlinkConflict { .. }
        )
    });
    if has_symlinks {
        println!("{}", bold("Symlinks:"));
        for step in plan {
            match step {
                PlanStep::SymlinkCreate(resource) => println!(
                    "  {} symlink {} -> {}",
                    green("+"),
                    display_target(&resource.target),
                    display_source(project, &resource.source),
                ),
                PlanStep::SymlinkUpdate(resource) => println!(
                    "  {} symlink {} -> {}",
                    yellow("~"),
                    display_target(&resource.target),
                    display_source(project, &resource.source),
                ),
                PlanStep::SymlinkRemove { target, .. } => {
                    println!("  {} symlink {}", red("-"), display_target(target))
                }
                PlanStep::SymlinkConflict { resource, reason } => println!(
                    "  {} symlink {} ({reason})",
                    red("!"),
                    display_target(&resource.target)
                ),
                _ => {}
            }
        }
    }

    let has_packages = plan.iter().any(|step| {
        matches!(
            step,
            PlanStep::PackageCreate { .. }
                | PlanStep::PackageRemove { .. }
                | PlanStep::PackageConflict { .. }
        )
    });
    if has_packages {
        if has_symlinks {
            println!();
        }
        println!("{}", bold("Packages:"));
        for step in plan {
            match step {
                PlanStep::PackageCreate { resource, .. } => {
                    println!("  {} {} {}", green("+"), resource.provider, resource.name,)
                }
                PlanStep::PackageRemove { resource, .. } => {
                    println!("  {} {} {}", red("-"), resource.provider, resource.name,)
                }
                PlanStep::PackageConflict { resource, reason } => println!(
                    "  {} {} {} ({reason})",
                    red("!"),
                    resource.provider,
                    resource.name,
                ),
                _ => {}
            }
        }
    }

    println!();
    println!(
        "{} {} to create, {} to update, {} to destroy{}",
        bold("Plan:"),
        green(&summary.create.to_string()),
        yellow(&summary.update.to_string()),
        red(&summary.remove.to_string()),
        if summary.conflicts > 0 {
            red(&format!(", {} conflicts", summary.conflicts))
        } else {
            ".".to_string()
        }
    );
}

fn print_state_initialized(project: &Project, state_path: &Path) {
    println!(
        "{} {}",
        dim("Initializing state:"),
        dim(&display_source(project, state_path))
    );
    println!();
}

fn apply_plan(plan: &[PlanStep], state: &mut State) -> Result<()> {
    let summary = summarize_plan(plan);
    if summary.conflicts > 0 {
        bail!(
            "plan has {} conflict(s); refusing to apply",
            summary.conflicts
        )
    }

    let tracked = track_noop_resources(plan, state);

    if summary.create + summary.update + summary.remove == 0 {
        if tracked > 0 {
            println!();
            println!("{} {} resources tracked.", bold("State updated:"), tracked);
        }
        return Ok(());
    }

    println!();
    println!("{}", bold("Applying:"));

    for step in plan {
        match step {
            PlanStep::SymlinkCreate(resource) => apply_with_status(
                "Creating",
                "Creation",
                &format!("symlink.{}", display_target(&resource.target)),
                || apply_symlink(resource, state),
            )?,
            PlanStep::SymlinkUpdate(resource) => apply_with_status(
                "Updating",
                "Update",
                &format!("symlink.{}", display_target(&resource.target)),
                || apply_symlink(resource, state),
            )?,
            PlanStep::SymlinkRemove { target, source } => {
                let resource = StateResource::Symlink {
                    target: target.clone(),
                    source: source.clone(),
                };
                apply_with_status(
                    "Destroying",
                    "Destroy",
                    &format!("symlink.{}", display_target(target)),
                    || remove_symlink(&resource, state),
                )?
            }
            PlanStep::PackageCreate { resource, provider } => apply_with_status(
                "Installing",
                "Install",
                &format!("package.{}.{}", resource.provider, resource.name),
                || install_package(provider, resource, state),
            )?,
            PlanStep::PackageRemove { resource, provider } => apply_with_status(
                "Removing",
                "Remove",
                &format!("package.{}.{}", resource.provider, resource.name),
                || remove_package(provider, resource, state),
            )?,
            PlanStep::SymlinkNoop(resource) => {
                state
                    .resources
                    .insert(symlink_id_for(resource), state_symlink(resource));
            }
            PlanStep::PackageNoop(resource) => {
                state
                    .resources
                    .insert(package_id_for(resource), state_package(resource));
            }
            PlanStep::SymlinkConflict { .. } | PlanStep::PackageConflict { .. } => unreachable!(),
        }
    }

    println!();
    println!(
        "{} {} created, {} updated, {} destroyed.",
        bold("Apply complete:"),
        green(&summary.create.to_string()),
        yellow(&summary.update.to_string()),
        red(&summary.remove.to_string()),
    );

    Ok(())
}

fn track_noop_resources(plan: &[PlanStep], state: &mut State) -> usize {
    let mut tracked = 0;
    for step in plan {
        match step {
            PlanStep::SymlinkNoop(resource) => {
                if state
                    .resources
                    .insert(symlink_id_for(resource), state_symlink(resource))
                    .is_none()
                {
                    tracked += 1;
                }
            }
            PlanStep::PackageNoop(resource) => {
                if state
                    .resources
                    .insert(package_id_for(resource), state_package(resource))
                    .is_none()
                {
                    tracked += 1;
                }
            }
            _ => {}
        }
    }
    tracked
}

fn apply_symlink(resource: &SymlinkResource, state: &mut State) -> Result<()> {
    if let Some(parent) = resource.target.parent() {
        fs::create_dir_all(parent)?;
    }
    if resource.target.exists() || fs::symlink_metadata(&resource.target).is_ok() {
        fs::remove_file(&resource.target)?;
    }
    unix_fs::symlink(&resource.source, &resource.target).with_context(|| {
        format!(
            "failed to symlink {} -> {}",
            resource.target.display(),
            resource.source.display()
        )
    })?;
    state
        .resources
        .insert(symlink_id_for(resource), state_symlink(resource));
    Ok(())
}

fn remove_symlink(resource: &StateResource, state: &mut State) -> Result<()> {
    let StateResource::Symlink { target, source } = resource else {
        return Ok(());
    };
    if fs::symlink_metadata(target)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
    {
        let current = fs::read_link(target)?;
        let current = resolve_symlink_target(target, &current);
        if same_path(&current, source) {
            fs::remove_file(target)?;
        }
    }
    state
        .resources
        .remove(&format!("symlink:{}", target.display()));
    Ok(())
}

fn install_package(
    provider: &PackageProvider,
    resource: &PackageResource,
    state: &mut State,
) -> Result<()> {
    if !package_provider_available(provider)? {
        bail!("{} is not available", resource.provider);
    }
    if !run_provider_command(&provider.install, Some(&resource.name), false)? {
        bail!("{} failed to install {}", resource.provider, resource.name);
    }
    state
        .resources
        .insert(package_id_for(resource), state_package(resource));
    Ok(())
}

fn remove_package(
    provider: &PackageProvider,
    resource: &PackageResource,
    state: &mut State,
) -> Result<()> {
    if !package_provider_available(provider)? {
        bail!("{} is not available", resource.provider);
    }
    if !run_provider_command(&provider.remove, Some(&resource.name), false)? {
        bail!("{} failed to remove {}", resource.provider, resource.name);
    }
    state.resources.remove(&package_id_for(resource));
    Ok(())
}

fn apply_with_status(
    action: &str,
    noun: &str,
    id: &str,
    apply: impl FnOnce() -> Result<()>,
) -> Result<()> {
    println!("  {id}: {}...", dim(action));
    match apply() {
        Ok(()) => {
            println!("  {id}: {}", green(&format!("{noun} complete")));
            Ok(())
        }
        Err(error) => {
            println!("  {id}: {}", red(&format!("{noun} failed")));
            Err(error)
        }
    }
}

fn colors_enabled() -> bool {
    std::io::stdout().is_terminal()
}

fn green(value: &str) -> String {
    if colors_enabled() {
        value.green().to_string()
    } else {
        value.to_string()
    }
}

fn yellow(value: &str) -> String {
    if colors_enabled() {
        value.yellow().to_string()
    } else {
        value.to_string()
    }
}

fn red(value: &str) -> String {
    if colors_enabled() {
        value.red().to_string()
    } else {
        value.to_string()
    }
}

fn bold(value: &str) -> String {
    if colors_enabled() {
        value.bold().to_string()
    } else {
        value.to_string()
    }
}

fn dim(value: &str) -> String {
    if colors_enabled() {
        value.dimmed().to_string()
    } else {
        value.to_string()
    }
}
