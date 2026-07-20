use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use mlua::{Lua, Table, Value};

use crate::command::CommandResource;
use crate::docker::ComposeResource;
use crate::font::{FontResource, expand_font_source};
use crate::managed_file::FileResource;
use crate::managed_output::{OutputDeclaration, ResourceAttributeReference, output_value_from_lua};
use crate::package::{
    PackageList, PackageListFormat, PackageMatcher, PackageProvider, PackageResource,
};
use crate::plan::package_id_for;
use crate::platform::platform_table;
use crate::project::Project;
use crate::service::{ServiceAction, ServiceProvider, ServiceResource};
use crate::ssh::{PassphrasePolicy, SshKeypairResource};
use crate::symlink::{
    SymlinkDeclaration, SymlinkResource, expand_home, expand_symlink_declaration, resolve_source,
    same_path,
};
use crate::systemd::SystemdUnitResource;
use crate::user::{SystemGroupResource, UserConfig, UserGroupResource, resolve_shell};

#[derive(Debug, Default)]
pub(crate) struct Config {
    pub(crate) symlinks: Vec<SymlinkResource>,
    pub(crate) symlink_declarations: Vec<SymlinkDeclaration>,
    pub(crate) packages: Vec<PackageResource>,
    pub(crate) services: Vec<ServiceResource>,
    pub(crate) systemd_units: Vec<SystemdUnitResource>,
    pub(crate) compose: Vec<ComposeResource>,
    pub(crate) fonts: Vec<FontResource>,
    pub(crate) files: Vec<FileResource>,
    pub(crate) commands: Vec<CommandResource>,
    pub(crate) outputs: Vec<OutputDeclaration>,
    pub(crate) ssh_keypairs: Vec<SshKeypairResource>,
    pub(crate) package_providers: BTreeMap<String, PackageProvider>,
    pub(crate) service_providers: BTreeMap<String, ServiceProvider>,
    pub(crate) user: UserConfig,
}

pub(crate) fn load_config(project: &Project, profile: &str) -> Result<Config> {
    let lua = Lua::new();
    let symlinks = lua.create_table()?;
    let packages = lua.create_table()?;
    let package_providers = lua.create_table()?;
    let service_providers = lua.create_table()?;
    let services = lua.create_table()?;
    let systemd_units = lua.create_table()?;
    let compose_resources = lua.create_table()?;
    let font_sources = lua.create_table()?;
    let files = lua.create_table()?;
    let commands = lua.create_table()?;
    let output_declarations = lua.create_table()?;
    let ssh_keypairs = lua.create_table()?;
    let system_groups = lua.create_table()?;
    let user_groups = lua.create_table()?;
    let user_shell = lua.create_table()?;
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

    let collected_font_sources = font_sources.clone();
    let fonts = lua.create_table()?;
    let fonts_install = lua.create_function(move |_, source: Option<String>| {
        collected_font_sources.raw_push(source.unwrap_or_else(|| "fonts".to_string()))?;
        Ok(())
    })?;
    fonts.set("install", fonts_install)?;

    let collected_files = files.clone();
    let root = project.root.clone();
    let file = lua.create_function(move |lua, (target, spec): (String, Table)| {
        let item = lua.create_table()?;
        item.set("target", expand_home(&target).display().to_string())?;
        let source = spec.get::<String>("source")?;
        item.set(
            "source",
            resolve_source(&root, &source).display().to_string(),
        )?;
        if let Some(mode) = spec.get::<Option<String>>("mode")? {
            let mode = u32::from_str_radix(&mode, 8)
                .map_err(|_| mlua::Error::RuntimeError(format!("invalid file mode: {mode}")))?;
            if mode > 0o7777 {
                return Err(mlua::Error::RuntimeError(format!(
                    "invalid file mode: {mode:o}"
                )));
            }
            item.set("mode", mode)?;
        }
        collected_files.raw_push(item)?;
        Ok(())
    })?;

    let collected_commands = commands.clone();
    let command = lua.create_function(move |lua, (name, spec): (String, Table)| {
        let item = lua.create_table()?;
        let id = format!("command:{name}");
        item.set("id", id.clone())?;
        item.set("name", name)?;
        item.set("check", spec.get::<String>("check")?)?;
        item.set("apply", spec.get::<String>("apply")?)?;
        if let Some(needs) = spec.get::<Option<Table>>("needs")? {
            item.set("needs", needs)?;
        }
        if let Some(provides) = spec.get::<Option<Table>>("provides")? {
            item.set("provides", provides)?;
        }
        collected_commands.raw_push(item)?;

        let reference = lua.create_table()?;
        reference.set("id", id)?;
        Ok(reference)
    })?;

    let ssh = lua.create_table()?;
    let collected_ssh_keypairs = ssh_keypairs.clone();
    let keypair = lua.create_function(move |lua, (name, spec): (String, Table)| {
        let path = expand_home(&spec.get::<String>("path")?);
        let passphrase = match spec.get::<bool>("passphrase")? {
            false => "none",
            true => "prompt",
        };
        let item = lua.create_table()?;
        item.set("name", name.clone())?;
        item.set("path", path.display().to_string())?;
        item.set("passphrase", passphrase)?;
        if let Some(comment) = spec.get::<Option<String>>("comment")? {
            item.set("comment", comment)?;
        }
        collected_ssh_keypairs.raw_push(item)?;

        let handle = lua.create_table()?;
        let id = format!("ssh-keypair:{name}");
        handle.set(
            "public_key",
            lua.create_userdata(ResourceAttributeReference {
                resource_id: id.clone(),
                attribute: "public_key".to_string(),
            })?,
        )?;
        handle.set(
            "fingerprint",
            lua.create_userdata(ResourceAttributeReference {
                resource_id: id,
                attribute: "fingerprint".to_string(),
            })?,
        )?;
        Ok(handle)
    })?;
    ssh.set("keypair", keypair)?;

    let collected_outputs = output_declarations.clone();
    let output = lua.create_function(move |lua, (name, spec): (String, Table)| {
        if name.is_empty() {
            return Err(mlua::Error::RuntimeError(
                "output name must not be empty".to_string(),
            ));
        }
        let value = spec.get::<Value>("value")?;
        if matches!(value, Value::Nil) {
            return Err(mlua::Error::RuntimeError(
                "output value is required".to_string(),
            ));
        }
        let item = lua.create_table()?;
        item.set("name", name)?;
        item.set("value", value)?;
        collected_outputs.raw_push(item)?;
        Ok(())
    })?;

    let group = lua.create_table()?;
    let collected_system_groups = system_groups.clone();
    let create_group = lua.create_function(move |lua, group_list: Table| {
        for name in table_strings(Some(group_list))? {
            let item = lua.create_table()?;
            item.set("name", name)?;
            collected_system_groups.raw_push(item)?;
        }
        Ok(())
    })?;
    group.set("create", create_group)?;

    let user = lua.create_table()?;
    let collected_user_shell = user_shell.clone();
    let shell = lua.create_function(move |_, shell: String| {
        collected_user_shell.set("name", shell)?;
        Ok(())
    })?;
    let collected_user_groups = user_groups.clone();
    let add_to_groups = lua.create_function(move |lua, group_list: Table| {
        for name in table_strings(Some(group_list))? {
            let item = lua.create_table()?;
            item.set("name", name)?;
            collected_user_groups.raw_push(item)?;
        }
        Ok(())
    })?;
    user.set("shell", shell)?;
    user.set("add_to_groups", add_to_groups)?;

    let platform = platform_table(&lua)?;

    let docker = lua.create_table()?;
    let collected_compose = compose_resources.clone();
    let root = project.root.clone();
    let compose = lua.create_function(move |lua, (name, spec): (String, Table)| {
        let item = lua.create_table()?;
        item.set("name", name)?;
        let file = spec.get::<String>("file")?;
        item.set("file", resolve_source(&root, &file).display().to_string())?;
        if let Some(profiles) = spec.get::<Option<Table>>("profiles")? {
            item.set("profiles", profiles)?;
        }
        if let Some(apply) = spec.get::<Option<Table>>("apply")? {
            item.set("apply", apply)?;
        }
        if let Some(remove) = spec.get::<Option<Table>>("remove")? {
            item.set("remove", remove)?;
        }
        collected_compose.raw_push(item)?;
        Ok(())
    })?;
    docker.set("compose", compose)?;

    dots.set("symlink", symlink)?;
    dots.set("docker", docker)?;
    dots.set("fonts", fonts)?;
    dots.set("file", file)?;
    dots.set("command", command)?;
    dots.set("output", output)?;
    dots.set("ssh", ssh)?;
    dots.set("group", group)?;
    dots.set("user", user)?;
    dots.set("profile", profile)?;
    dots.set("platform", platform)?;
    dots.set("root", project.root.display().to_string())?;
    dots.set("os", std::env::consts::OS)?;

    install_provider_api(
        &lua,
        &dots,
        package_providers.clone(),
        packages.clone(),
        service_providers.clone(),
        services.clone(),
    )?;

    let package: Table = lua.globals().get("package")?;
    let preload: Table = package.get("preload")?;
    let dots_module = dots.clone();
    preload.set(
        "dots",
        lua.create_function(move |_, ()| Ok(dots_module.clone()))?,
    )?;
    lua.globals().set("dots", &dots)?;

    install_package_path(&lua, &project.root)?;
    load_builtin_lua(&lua)?;
    install_systemd_unit_api(&lua, &dots, systemd_units.clone(), project.root.clone())?;

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
        if declaration.source.is_dir() {
            config.symlink_declarations.push(declaration.clone());
        }
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
    for item in services.sequence_values::<Table>() {
        let item = item?;
        let action = match item.get::<String>("action")?.as_str() {
            "start" => ServiceAction::Start,
            "enable" => ServiceAction::Enable,
            action => bail!("unsupported service action: {action}"),
        };
        config.services.push(ServiceResource {
            provider: item.get::<String>("provider")?,
            action,
            name: item.get::<String>("name")?,
        });
    }
    for item in systemd_units.sequence_values::<Table>() {
        let item = item?;
        config.systemd_units.push(SystemdUnitResource {
            unit: item.get::<String>("unit")?,
            file: PathBuf::from(item.get::<String>("file")?),
        });
    }
    for item in compose_resources.sequence_values::<Table>() {
        let item = item?;
        config.compose.push(ComposeResource {
            name: item.get::<String>("name")?,
            file: PathBuf::from(item.get::<String>("file")?),
            profiles: table_strings(item.get::<Option<Table>>("profiles")?)?,
            apply: match item.get::<Option<Table>>("apply")? {
                Some(args) => table_strings(Some(args))?,
                None => vec!["up".to_string(), "--detach".to_string()],
            },
            remove: match item.get::<Option<Table>>("remove")? {
                Some(args) => table_strings(Some(args))?,
                None => vec!["down".to_string()],
            },
        });
    }
    for source in font_sources.sequence_values::<String>() {
        config
            .fonts
            .extend(expand_font_source(&project.root, Some(&source?))?);
    }
    for item in files.sequence_values::<Table>() {
        let item = item?;
        config.files.push(FileResource {
            target: PathBuf::from(item.get::<String>("target")?),
            source: PathBuf::from(item.get::<String>("source")?),
            mode: item.get::<Option<u32>>("mode")?,
        });
    }
    for item in commands.sequence_values::<Table>() {
        let item = item?;
        config.commands.push(CommandResource {
            name: item.get::<String>("name")?,
            check: item.get::<String>("check")?,
            apply: item.get::<String>("apply")?,
            needs: table_references(item.get::<Option<Table>>("needs")?)?,
            provides: table_references(item.get::<Option<Table>>("provides")?)?,
        });
    }
    for item in ssh_keypairs.sequence_values::<Table>() {
        let item = item?;
        config.ssh_keypairs.push(SshKeypairResource {
            name: item.get::<String>("name")?,
            private_path: PathBuf::from(item.get::<String>("path")?),
            comment: item.get::<Option<String>>("comment")?,
            passphrase: match item.get::<String>("passphrase")?.as_str() {
                "none" => PassphrasePolicy::None,
                "prompt" => PassphrasePolicy::Prompt,
                _ => unreachable!(),
            },
        });
    }
    let mut output_names = BTreeSet::new();
    for item in output_declarations.sequence_values::<Table>() {
        let item = item?;
        let name = item.get::<String>("name")?;
        if !output_names.insert(name.clone()) {
            bail!("duplicate output: {name}");
        }
        config.outputs.push(OutputDeclaration {
            name,
            value: output_value_from_lua(item.get::<Value>("value")?)?,
        });
    }
    if let Some(shell) = user_shell.get::<Option<String>>("name")? {
        config.user.shell = Some(resolve_shell(&shell)?);
    }
    for item in system_groups.sequence_values::<Table>() {
        let item = item?;
        config.user.groups.push(SystemGroupResource {
            name: item.get::<String>("name")?,
        });
    }
    for item in user_groups.sequence_values::<Table>() {
        let item = item?;
        config.user.memberships.push(UserGroupResource {
            name: item.get::<String>("name")?,
        });
    }
    for pair in package_providers.pairs::<String, Table>() {
        let (name, item) = pair?;
        config
            .package_providers
            .insert(name.clone(), package_provider_from_table(&name, &item)?);
    }
    for pair in service_providers.pairs::<String, Table>() {
        let (name, item) = pair?;
        config.service_providers.insert(
            name.clone(),
            ServiceProvider {
                capability: item
                    .get::<Option<String>>("capability")?
                    .unwrap_or_else(|| format!("provider:{name}")),
                available: item
                    .get::<Option<String>>("available")?
                    .unwrap_or_else(|| "exit 0".to_string()),
                started: item.get::<Option<String>>("started")?,
                start: item.get::<Option<String>>("start")?,
                stop: item.get::<Option<String>>("stop")?,
                enabled: item.get::<Option<String>>("enabled")?,
                enable: item.get::<Option<String>>("enable")?,
                disable: item.get::<Option<String>>("disable")?,
                list_started: item.get::<Option<String>>("list_started")?,
                list_enabled: item.get::<Option<String>>("list_enabled")?,
            },
        );
    }
    dedupe_config(&mut config)?;
    Ok(config)
}

fn package_provider_from_table(name: &str, item: &Table) -> Result<PackageProvider> {
    let list = match item.get::<Option<Value>>("list")? {
        Some(Value::String(command)) => Some(PackageList {
            command: command.to_string_lossy(),
            format: PackageListFormat::Lines,
        }),
        Some(Value::Table(table)) => {
            let format = table
                .get::<Option<String>>("format")?
                .unwrap_or_else(|| PackageListFormat::Lines.as_str().to_string());
            Some(PackageList {
                command: table.get::<String>("command")?,
                format: PackageListFormat::from_str(&format)
                    .ok_or_else(|| anyhow::anyhow!("unsupported package list format: {format}"))?,
            })
        }
        Some(_) => bail!("package provider {name} list must be a string or table"),
        None => None,
    };

    let package_provides = match item.get::<Option<Table>>("package_provides")? {
        Some(table) => table
            .pairs::<String, String>()
            .collect::<mlua::Result<BTreeMap<_, _>>>()?,
        None => BTreeMap::new(),
    };

    let matcher = item
        .get::<Option<String>>("match")?
        .map(|matcher| {
            PackageMatcher::from_str(&matcher)
                .ok_or_else(|| anyhow::anyhow!("unsupported package matcher: {matcher}"))
        })
        .transpose()?
        .unwrap_or(PackageMatcher::Exact);

    Ok(PackageProvider {
        capability: item
            .get::<Option<String>>("capability")?
            .unwrap_or_else(|| format!("provider:{name}")),
        available: item.get::<String>("available")?,
        installed: item.get::<String>("installed")?,
        install: item.get::<String>("install")?,
        remove: item.get::<String>("remove")?,
        list,
        package_provides,
        matcher,
    })
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

    let symlink_targets = config
        .symlinks
        .iter()
        .map(|resource| resource.target.clone())
        .collect::<BTreeSet<_>>();
    let mut files = BTreeMap::<PathBuf, FileResource>::new();
    for resource in config.files.drain(..) {
        if symlink_targets.contains(&resource.target) {
            bail!(
                "target is declared as both file and symlink: {}",
                resource.target.display()
            );
        }
        match files.get(&resource.target) {
            Some(existing) if existing == &resource => {}
            Some(_) => bail!("duplicate file target: {}", resource.target.display()),
            None => {
                files.insert(resource.target.clone(), resource);
            }
        }
    }
    config.files = files.into_values().collect();

    let mut package_ids = BTreeSet::new();
    config
        .packages
        .retain(|resource| package_ids.insert(package_id_for(resource)));

    let mut service_ids = BTreeSet::new();
    config
        .services
        .retain(|resource| service_ids.insert(crate::plan::service_id_for(resource)));

    let mut systemd_units = BTreeMap::<String, SystemdUnitResource>::new();
    let mut systemd_unit_resources = Vec::new();
    for resource in config.systemd_units.drain(..) {
        match systemd_units.get(&resource.unit) {
            Some(existing) if existing == &resource => {}
            Some(_) => bail!("duplicate systemd unit: {}", resource.unit),
            None => {
                systemd_units.insert(resource.unit.clone(), resource.clone());
                systemd_unit_resources.push(resource);
            }
        }
    }
    config.systemd_units = systemd_unit_resources;

    let mut compose_names = BTreeMap::<String, ComposeResource>::new();
    let mut compose = Vec::new();
    for resource in config.compose.drain(..) {
        match compose_names.get(&resource.name) {
            Some(existing) if existing == &resource => {}
            Some(_) => bail!("duplicate Docker Compose application: {}", resource.name),
            None => {
                compose_names.insert(resource.name.clone(), resource.clone());
                compose.push(resource);
            }
        }
    }
    config.compose = compose;

    let mut font_ids = BTreeSet::new();
    config
        .fonts
        .retain(|resource| font_ids.insert(crate::plan::font_id_for(resource)));

    let mut keypair_names = BTreeSet::new();
    let mut keypair_paths = BTreeSet::new();
    for resource in &config.ssh_keypairs {
        if !keypair_names.insert(resource.name.clone()) {
            bail!("duplicate SSH keypair: {}", resource.name);
        }
        if !keypair_paths.insert(resource.private_path.clone()) {
            bail!(
                "duplicate SSH keypair path: {}",
                resource.private_path.display()
            );
        }
    }

    let mut command_names = BTreeMap::<String, CommandResource>::new();
    let mut commands = Vec::new();
    for resource in config.commands.drain(..) {
        match command_names.get(&resource.name) {
            Some(existing)
                if existing.check == resource.check
                    && existing.apply == resource.apply
                    && existing.needs == resource.needs
                    && existing.provides == resource.provides => {}
            Some(_) => bail!("duplicate command: {}", resource.name),
            None => {
                command_names.insert(resource.name.clone(), resource.clone());
                commands.push(resource);
            }
        }
    }
    config.commands = commands;

    let mut group_names = BTreeSet::new();
    config
        .user
        .groups
        .retain(|resource| group_names.insert(resource.name.clone()));

    let mut membership_names = BTreeSet::new();
    config
        .user
        .memberships
        .retain(|resource| membership_names.insert(resource.name.clone()));

    Ok(())
}

fn install_provider_api(
    lua: &Lua,
    dots: &Table,
    providers: Table,
    packages: Table,
    service_providers: Table,
    services: Table,
) -> Result<()> {
    let provider_api = lua.create_table()?;
    let dots_for_package = dots.clone();
    let package = lua.create_function(move |lua, (name, spec): (String, Table)| {
        let provider = package_provider_from_table(&name, &spec).map_err(mlua::Error::external)?;
        register_package_provider(
            lua,
            &dots_for_package,
            &providers,
            packages.clone(),
            &name,
            provider,
        )
        .map_err(mlua::Error::external)
    })?;
    provider_api.set("package", package)?;

    let dots_for_service = dots.clone();
    let service = lua.create_function(move |lua, (name, spec): (String, Table)| {
        let provider = ServiceProvider {
            capability: spec
                .get::<Option<String>>("capability")?
                .unwrap_or_else(|| format!("provider:{name}")),
            available: spec
                .get::<Option<String>>("available")?
                .unwrap_or_else(|| "exit 0".to_string()),
            started: spec.get("started")?,
            start: spec.get("start")?,
            stop: spec.get("stop")?,
            enabled: spec.get("enabled")?,
            enable: spec.get("enable")?,
            disable: spec.get("disable")?,
            list_started: spec.get("list_started")?,
            list_enabled: spec.get("list_enabled")?,
        };
        register_service_provider(
            lua,
            &dots_for_service,
            &service_providers,
            services.clone(),
            &name,
            provider,
        )
        .map_err(mlua::Error::external)
    })?;
    provider_api.set("service", service)?;
    dots.set("provider", provider_api)?;
    Ok(())
}

fn load_builtin_lua(lua: &Lua) -> Result<()> {
    for (name, source) in [
        (
            "dots package pacman",
            include_str!("lua/packages/pacman.lua"),
        ),
        ("dots package paru", include_str!("lua/packages/paru.lua")),
        ("dots package yay", include_str!("lua/packages/yay.lua")),
        ("dots package apt", include_str!("lua/packages/apt.lua")),
        ("dots package dnf", include_str!("lua/packages/dnf.lua")),
        (
            "dots package zypper",
            include_str!("lua/packages/zypper.lua"),
        ),
        ("dots package apk", include_str!("lua/packages/apk.lua")),
        (
            "dots package flatpak",
            include_str!("lua/packages/flatpak.lua"),
        ),
        ("dots package snap", include_str!("lua/packages/snap.lua")),
        ("dots package brew", include_str!("lua/packages/brew.lua")),
        (
            "dots service systemd",
            include_str!("lua/services/systemd.lua"),
        ),
        (
            "dots service openrc",
            include_str!("lua/services/openrc.lua"),
        ),
        ("dots service brew", include_str!("lua/services/brew.lua")),
    ] {
        lua.load(source).set_name(name).exec()?;
    }
    Ok(())
}

fn install_systemd_unit_api(lua: &Lua, dots: &Table, units: Table, root: PathBuf) -> Result<()> {
    let systemd: Table = dots.get("systemd")?;
    let install = lua.create_function(move |lua, files: Table| {
        for file in table_strings(Some(files)).map_err(mlua::Error::external)? {
            let source = resolve_source(&root, &file);
            let unit = source
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| {
                    mlua::Error::RuntimeError(format!("invalid systemd unit file: {file}"))
                })?;
            let item = lua.create_table()?;
            item.set("unit", unit)?;
            item.set("file", source.display().to_string())?;
            units.raw_push(item)?;
        }
        Ok(())
    })?;
    systemd.set("install", install)?;
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
    spec.set("capability", provider.capability)?;
    spec.set("available", provider.available)?;
    spec.set("installed", provider.installed)?;
    spec.set("install", provider.install)?;
    spec.set("remove", provider.remove)?;
    if let Some(list) = provider.list {
        let table = lua.create_table()?;
        table.set("command", list.command)?;
        table.set("format", list.format.as_str())?;
        spec.set("list", table)?;
    }
    if !provider.package_provides.is_empty() {
        let table = lua.create_table()?;
        for (package, capability) in provider.package_provides {
            table.set(package, capability)?;
        }
        spec.set("package_provides", table)?;
    }
    spec.set("match", provider.matcher.as_str())?;
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

fn register_service_provider(
    lua: &Lua,
    dots: &Table,
    providers: &Table,
    services: Table,
    name: &str,
    provider: ServiceProvider,
) -> Result<()> {
    let spec = lua.create_table()?;
    spec.set("capability", provider.capability)?;
    spec.set("available", provider.available)?;
    spec.set("started", provider.started)?;
    spec.set("start", provider.start)?;
    spec.set("stop", provider.stop)?;
    spec.set("enabled", provider.enabled)?;
    spec.set("enable", provider.enable)?;
    spec.set("disable", provider.disable)?;
    spec.set("list_started", provider.list_started)?;
    spec.set("list_enabled", provider.list_enabled)?;
    providers.set(name, spec)?;
    dots.set(
        name,
        service_provider_table(lua, services, name.to_string())?,
    )?;
    Ok(())
}

fn service_provider_table(lua: &Lua, services: Table, provider: String) -> Result<Table> {
    let table = lua.create_table()?;
    let start = service_action(
        lua,
        services.clone(),
        provider.clone(),
        ServiceAction::Start,
    )?;
    let enable = service_action(lua, services, provider, ServiceAction::Enable)?;
    table.set("start", start)?;
    table.set("enable", enable)?;
    Ok(table)
}

fn service_action(
    lua: &Lua,
    services: Table,
    provider: String,
    action: ServiceAction,
) -> Result<mlua::Function> {
    Ok(lua.create_function(move |lua, service_list: Table| {
        for name in table_strings(Some(service_list))? {
            let item = lua.create_table()?;
            item.set("provider", provider.clone())?;
            item.set("action", action.as_str())?;
            item.set("name", name)?;
            services.raw_push(item)?;
        }
        Ok(())
    })?)
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
                    "expected strings, got {other:?}"
                )));
            }
        }
    }
    Ok(values)
}

fn table_references(table: Option<Table>) -> mlua::Result<Vec<String>> {
    let Some(table) = table else {
        return Ok(Vec::new());
    };
    let mut values = Vec::new();
    for value in table.sequence_values::<Value>() {
        match value? {
            Value::String(value) => values.push(value.to_string_lossy()),
            Value::Table(table) => values.push(table.get::<String>("id")?),
            other => {
                return Err(mlua::Error::RuntimeError(format!(
                    "expected strings or resource references, got {other:?}"
                )));
            }
        }
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    struct TempProject {
        _dir: tempfile::TempDir,
        project: Project,
    }

    impl std::ops::Deref for TempProject {
        type Target = Project;

        fn deref(&self) -> &Self::Target {
            &self.project
        }
    }

    fn temp_project(config: &str) -> TempProject {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let dir = tempfile::Builder::new()
            .prefix(&format!("dots-config-test-{id}-"))
            .tempdir()
            .unwrap();
        let root = dir.path().to_path_buf();
        let config_path = root.join("dots.lua");
        fs::write(&config_path, config).unwrap();
        TempProject {
            _dir: dir,
            project: Project {
                root,
                config: config_path,
            },
        }
    }

    #[test]
    fn resolves_symlink_paths_from_config() {
        let project = temp_project(r#"dots.symlink("~/.zshrc", ".zshrc")"#);
        fs::write(project.root.join(".zshrc"), "").unwrap();

        let config = load_config(&project, "test").unwrap();

        assert_eq!(config.symlinks.len(), 1);
        assert_eq!(config.symlinks[0].target, expand_home("~/.zshrc"));
        assert_eq!(config.symlinks[0].source, project.root.join(".zshrc"));
    }

    #[test]
    fn duplicate_symlink_target_with_same_source_is_deduped() {
        let project = temp_project(
            r#"
            dots.symlink("TARGET", "source")
            dots.symlink("TARGET", "source")
            "#,
        );
        fs::write(project.root.join("source"), "").unwrap();
        let source = fs::read_to_string(&project.config)
            .unwrap()
            .replace("TARGET", &project.root.join("target").display().to_string());
        fs::write(&project.config, source).unwrap();

        let config = load_config(&project, "test").unwrap();

        assert_eq!(config.symlinks.len(), 1);
    }

    #[test]
    fn duplicate_symlink_target_with_different_source_errors() {
        let project = temp_project(
            r#"
            dots.symlink("TARGET", "one")
            dots.symlink("TARGET", "two")
            "#,
        );
        fs::write(project.root.join("one"), "").unwrap();
        fs::write(project.root.join("two"), "").unwrap();
        let source = fs::read_to_string(&project.config)
            .unwrap()
            .replace("TARGET", &project.root.join("target").display().to_string());
        fs::write(&project.config, source).unwrap();

        let error = load_config(&project, "test").unwrap_err().to_string();

        assert!(error.contains("duplicate symlink target"));
    }

    #[test]
    fn expands_stow_style_directory_and_applies_ignore() {
        let project = temp_project(
            r#"
            dots.symlink("TARGET", "home", {
              ignore = { "skip/**" },
            })
            "#,
        );
        let target = project.root.join("target-home");
        fs::create_dir_all(project.root.join("home/.config/nvim")).unwrap();
        fs::create_dir_all(project.root.join("home/skip")).unwrap();
        fs::create_dir_all(target.join(".config")).unwrap();
        fs::write(project.root.join("home/.config/nvim/init.lua"), "").unwrap();
        fs::write(project.root.join("home/skip/file"), "").unwrap();
        let source = fs::read_to_string(&project.config)
            .unwrap()
            .replace("TARGET", &target.display().to_string());
        fs::write(&project.config, source).unwrap();

        let config = load_config(&project, "test").unwrap();

        assert!(config.symlinks.iter().any(|resource| {
            resource.target == target.join(".config/nvim")
                && resource.source == project.root.join("home/.config/nvim")
        }));
        assert!(
            config
                .symlinks
                .iter()
                .all(|resource| !resource.target.starts_with(target.join("skip")))
        );
    }

    #[test]
    fn loads_repo_local_lua_modules() {
        let project = temp_project(r#"require("dots.common")"#);
        fs::create_dir_all(project.root.join("dots")).unwrap();
        fs::write(
            project.root.join("dots/common.lua"),
            r#"dots.paru.install({ "bat" })"#,
        )
        .unwrap();

        let config = load_config(&project, "test").unwrap();

        assert_eq!(config.packages.len(), 1);
        assert_eq!(config.packages[0].provider, "paru");
        assert_eq!(config.packages[0].name, "bat");
    }

    #[test]
    fn loads_yay_packages() {
        let project = temp_project(r#"dots.yay.install({ "fd" })"#);

        let config = load_config(&project, "test").unwrap();

        assert!(config.package_providers.contains_key("yay"));
        assert_eq!(config.packages.len(), 1);
        assert_eq!(config.packages[0].provider, "yay");
        assert_eq!(config.packages[0].name, "fd");
    }

    #[test]
    fn loads_additional_builtin_package_providers() {
        let project = temp_project(
            r#"
            dots.dnf.install({ "fd-find" })
            dots.zypper.install({ "fd" })
            dots.apk.install({ "ripgrep" })
            dots.flatpak.install({ "org.mozilla.firefox" })
            dots.snap.install({ "firefox" })
            "#,
        );

        let config = load_config(&project, "test").unwrap();

        for provider in ["dnf", "zypper", "apk", "flatpak", "snap"] {
            assert!(config.package_providers.contains_key(provider));
            assert!(
                config
                    .packages
                    .iter()
                    .any(|package| package.provider == provider)
            );
        }
        assert_eq!(
            config.package_providers["flatpak"].remove,
            "flatpak uninstall --assumeyes --noninteractive \"$DOTS_PACKAGE\""
        );
    }

    #[test]
    fn duplicate_packages_are_deduped() {
        let project = temp_project(r#"dots.paru.install({ "bat", "bat" })"#);

        let config = load_config(&project, "test").unwrap();

        assert_eq!(config.packages.len(), 1);
        assert_eq!(config.packages[0].provider, "paru");
        assert_eq!(config.packages[0].name, "bat");
    }

    #[test]
    fn package_order_is_preserved() {
        let project = temp_project(
            r#"
            dots.brew.tap({ "example/tools" })
            dots.brew.trust.tap({ "example/tools" })
            dots.brew.install({ "example/tools/widget" })
            "#,
        );

        let config = load_config(&project, "test").unwrap();

        assert_eq!(config.packages.len(), 3);
        assert_eq!(config.packages[0].provider, "brew-tap");
        assert_eq!(config.packages[0].name, "example/tools");
        assert_eq!(config.packages[1].provider, "brew-trusted-tap");
        assert_eq!(config.packages[1].name, "example/tools");
        assert_eq!(config.packages[2].provider, "brew");
        assert_eq!(config.packages[2].name, "example/tools/widget");
    }

    #[test]
    fn loads_default_fonts_folder() {
        let project = temp_project(r#"dots.fonts.install()"#);
        fs::create_dir_all(project.root.join("fonts/nested")).unwrap();
        fs::write(project.root.join("fonts/runcat.ttf"), "font").unwrap();
        fs::write(project.root.join("fonts/nested/readme.txt"), "nope").unwrap();

        let config = load_config(&project, "test").unwrap();

        assert_eq!(config.fonts.len(), 1);
        assert_eq!(
            config.fonts[0].source,
            project.root.join("fonts/runcat.ttf")
        );
        assert!(config.fonts[0].target.ends_with("runcat.ttf"));
    }

    #[test]
    fn loads_brew_casks() {
        let project = temp_project(r#"dots.brew.cask({ "ghostty" })"#);

        let config = load_config(&project, "test").unwrap();

        assert!(config.package_providers.contains_key("brew-cask"));
        assert_eq!(config.packages.len(), 1);
        assert_eq!(config.packages[0].provider, "brew-cask");
        assert_eq!(config.packages[0].name, "ghostty");
    }

    #[test]
    fn loads_brew_formula_trust() {
        let project = temp_project(r#"dots.brew.trust.formula({ "example/tools/widget" })"#);

        let config = load_config(&project, "test").unwrap();

        assert!(
            config
                .package_providers
                .contains_key("brew-trusted-formula")
        );
        assert_eq!(config.packages.len(), 1);
        assert_eq!(config.packages[0].provider, "brew-trusted-formula");
        assert_eq!(config.packages[0].name, "example/tools/widget");
    }

    #[test]
    fn loads_brew_taps() {
        let project = temp_project(r#"dots.brew.tap({ "FelixKratz/formulae" })"#);

        let config = load_config(&project, "test").unwrap();

        assert!(config.package_providers.contains_key("brew-tap"));
        assert_eq!(config.packages.len(), 1);
        assert_eq!(config.packages[0].provider, "brew-tap");
        assert_eq!(config.packages[0].name, "FelixKratz/formulae");
    }

    #[test]
    fn loads_services() {
        let project = temp_project(
            r#"
            dots.systemd.enable({ "docker.service" })
            dots.systemd.start({ "docker.service" })
            dots.brew.service.start({ "sketchybar" })
            "#,
        );

        let config = load_config(&project, "test").unwrap();

        assert!(config.service_providers.contains_key("systemd"));
        let systemd = &config.service_providers["systemd"];
        assert!(systemd.list_started.as_deref().is_some_and(|command| {
            command.contains("--state=active") && !command.contains("--type=service")
        }));
        assert!(
            systemd
                .list_enabled
                .as_deref()
                .is_some_and(|command| !command.contains("--type=service"))
        );
        assert!(config.service_providers.contains_key("brew-service"));
        assert_eq!(config.services.len(), 3);
        assert!(config.services.iter().any(|service| {
            service.provider == "systemd"
                && service.action == ServiceAction::Enable
                && service.name == "docker.service"
        }));
        assert!(config.services.iter().any(|service| {
            service.provider == "systemd"
                && service.action == ServiceAction::Start
                && service.name == "docker.service"
        }));
        assert!(config.services.iter().any(|service| {
            service.provider == "brew-service"
                && service.action == ServiceAction::Start
                && service.name == "sketchybar"
        }));
    }

    #[test]
    fn loads_managed_systemd_services() {
        let project = temp_project(
            r#"
            dots.systemd.install({
              "services/my-service.service",
              "systemd/automatic-timezone.timer",
            })
            "#,
        );

        let config = load_config(&project, "test").unwrap();

        assert_eq!(config.systemd_units.len(), 2);
        assert_eq!(config.systemd_units[0].unit, "my-service.service");
        assert_eq!(
            config.systemd_units[0].file,
            project.root.join("services/my-service.service")
        );
        assert_eq!(config.systemd_units[1].unit, "automatic-timezone.timer");
    }

    #[test]
    fn loads_docker_compose_applications() {
        let project = temp_project(
            r#"
            dots.docker.compose("my-service", {
              file = "services/my-service/compose.yaml",
              profiles = { "production" },
              apply = { "up", "--detach", "--wait" },
              remove = { "down", "--remove-orphans" },
            })
            "#,
        );

        let config = load_config(&project, "test").unwrap();
        let resource = &config.compose[0];

        assert_eq!(resource.name, "my-service");
        assert_eq!(
            resource.file,
            project.root.join("services/my-service/compose.yaml")
        );
        assert_eq!(resource.profiles, vec!["production"]);
        assert_eq!(resource.apply, vec!["up", "--detach", "--wait"]);
        assert_eq!(resource.remove, vec!["down", "--remove-orphans"]);
    }

    #[test]
    fn docker_compose_uses_safe_defaults() {
        let project =
            temp_project(r#"dots.docker.compose("my-service", { file = "compose.yaml" })"#);

        let config = load_config(&project, "test").unwrap();
        let resource = &config.compose[0];

        assert_eq!(resource.apply, vec!["up", "--detach"]);
        assert_eq!(resource.remove, vec!["down"]);
    }

    #[test]
    fn loads_openrc_services() {
        let project = temp_project(
            r#"
            dots.openrc.enable({ "docker" })
            dots.openrc.start({ "docker" })
            "#,
        );

        let config = load_config(&project, "test").unwrap();

        assert!(config.service_providers.contains_key("openrc"));
        let provider = &config.service_providers["openrc"];
        assert_eq!(provider.list_started, None);
        assert_eq!(
            provider.list_enabled.as_deref(),
            Some("rc-update show default | awk '{ print $1 }'")
        );
        assert_eq!(config.services.len(), 2);
        assert!(config.services.iter().any(|service| {
            service.provider == "openrc"
                && service.action == ServiceAction::Enable
                && service.name == "docker"
        }));
        assert!(config.services.iter().any(|service| {
            service.provider == "openrc"
                && service.action == ServiceAction::Start
                && service.name == "docker"
        }));
    }

    #[test]
    fn loads_user_settings() {
        let project = temp_project(
            r#"
            dots.user.shell("sh")
            dots.group.create({ "media", "media" })
            dots.user.add_to_groups({ "docker", "docker" })
            "#,
        );

        let config = load_config(&project, "test").unwrap();

        let shell = config.user.shell.unwrap();
        assert_eq!(shell.name, "sh");
        assert_eq!(config.user.groups.len(), 1);
        assert_eq!(config.user.groups[0].name, "media");
        assert_eq!(config.user.memberships.len(), 1);
        assert_eq!(config.user.memberships[0].name, "docker");
    }

    #[test]
    fn loads_commands_in_order() {
        let project = temp_project(
            r#"
            dots.command("mise", { check = "exit 1", apply = "exit 0" })
            dots.command("pi", { check = "exit 1", apply = "exit 0" })
            "#,
        );

        let config = load_config(&project, "test").unwrap();

        assert_eq!(config.commands.len(), 2);
        assert_eq!(config.commands[0].name, "mise");
        assert_eq!(config.commands[1].name, "pi");
    }

    #[test]
    fn command_returns_reference_for_needs() {
        let project = temp_project(
            r#"
            local node = dots.command("node", {
              check = "exit 1",
              apply = "exit 0",
            })
            dots.command("prettier", {
              check = "exit 1",
              apply = "exit 0",
              needs = { node },
            })
            "#,
        );

        let config = load_config(&project, "test").unwrap();

        assert_eq!(config.commands[1].needs, vec!["command:node"]);
    }

    #[test]
    fn loads_lua_package_provider() {
        let project = temp_project(
            r#"
            dots.provider.package("fake", {
              available = "exit 0",
              installed = "exit 1",
              install = "exit 0",
              remove = "exit 0",
            })

            dots.fake.install({ "one", "two" })
            "#,
        );

        let config = load_config(&project, "test").unwrap();

        assert!(config.package_providers.contains_key("fake"));
        assert_eq!(config.packages.len(), 2);
        assert!(
            config
                .packages
                .iter()
                .any(|package| package.provider == "fake" && package.name == "one")
        );
        assert!(
            config
                .packages
                .iter()
                .any(|package| package.provider == "fake" && package.name == "two")
        );
    }
}
