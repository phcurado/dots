use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use mlua::{Lua, Table, Value};

use crate::package::{PackageProvider, PackageResource};
use crate::plan::package_id_for;
use crate::platform::platform_table;
use crate::project::Project;
use crate::service::{ServiceAction, ServiceProvider, ServiceResource};
use crate::symlink::{
    SymlinkDeclaration, SymlinkResource, expand_home, expand_symlink_declaration, resolve_source,
    same_path,
};

#[derive(Debug, Default)]
pub(crate) struct Config {
    pub(crate) symlinks: Vec<SymlinkResource>,
    pub(crate) packages: Vec<PackageResource>,
    pub(crate) services: Vec<ServiceResource>,
    pub(crate) package_providers: BTreeMap<String, PackageProvider>,
    pub(crate) service_providers: BTreeMap<String, ServiceProvider>,
}

pub(crate) fn load_config(project: &Project, profile: &str) -> Result<Config> {
    let lua = Lua::new();
    let symlinks = lua.create_table()?;
    let packages = lua.create_table()?;
    let package_providers = lua.create_table()?;
    let service_providers = lua.create_table()?;
    let services = lua.create_table()?;
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
    for pair in service_providers.pairs::<String, Table>() {
        let (name, item) = pair?;
        config.service_providers.insert(
            name,
            ServiceProvider {
                started: item.get::<Option<String>>("started")?,
                start: item.get::<Option<String>>("start")?,
                stop: item.get::<Option<String>>("stop")?,
                enabled: item.get::<Option<String>>("enabled")?,
                enable: item.get::<Option<String>>("enable")?,
                disable: item.get::<Option<String>>("disable")?,
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

    let mut package_ids = BTreeSet::new();
    config
        .packages
        .retain(|resource| package_ids.insert(package_id_for(resource)));

    let mut service_ids = BTreeSet::new();
    config
        .services
        .retain(|resource| service_ids.insert(crate::plan::service_id_for(resource)));

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
        let provider = PackageProvider {
            available: spec.get("available")?,
            installed: spec.get("installed")?,
            install: spec.get("install")?,
            remove: spec.get("remove")?,
        };
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
            started: spec.get("started")?,
            start: spec.get("start")?,
            stop: spec.get("stop")?,
            enabled: spec.get("enabled")?,
            enable: spec.get("enable")?,
            disable: spec.get("disable")?,
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
        ("dots packages", include_str!("lua/packages.lua")),
        ("dots services", include_str!("lua/services.lua")),
    ] {
        lua.load(source).set_name(name).exec()?;
    }
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

fn register_service_provider(
    lua: &Lua,
    dots: &Table,
    providers: &Table,
    services: Table,
    name: &str,
    provider: ServiceProvider,
) -> Result<()> {
    let spec = lua.create_table()?;
    spec.set("started", provider.started)?;
    spec.set("start", provider.start)?;
    spec.set("stop", provider.stop)?;
    spec.set("enabled", provider.enabled)?;
    spec.set("enable", provider.enable)?;
    spec.set("disable", provider.disable)?;
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
                    "ignore patterns must be strings, got {other:?}"
                )));
            }
        }
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_project(config: &str) -> Project {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("dots-config-test-{}-{id}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let config_path = root.join("dots.lua");
        fs::write(&config_path, config).unwrap();
        Project {
            root,
            config: config_path,
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
            dots.symlink("/tmp/dots-test-target", "source")
            dots.symlink("/tmp/dots-test-target", "source")
            "#,
        );
        fs::write(project.root.join("source"), "").unwrap();

        let config = load_config(&project, "test").unwrap();

        assert_eq!(config.symlinks.len(), 1);
    }

    #[test]
    fn duplicate_symlink_target_with_different_source_errors() {
        let project = temp_project(
            r#"
            dots.symlink("/tmp/dots-test-target", "one")
            dots.symlink("/tmp/dots-test-target", "two")
            "#,
        );
        fs::write(project.root.join("one"), "").unwrap();
        fs::write(project.root.join("two"), "").unwrap();

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
            dots.brew.tap({ "FelixKratz/formulae" })
            dots.brew.install({ "sketchybar" })
            "#,
        );

        let config = load_config(&project, "test").unwrap();

        assert_eq!(config.packages.len(), 2);
        assert_eq!(config.packages[0].provider, "brew-tap");
        assert_eq!(config.packages[0].name, "FelixKratz/formulae");
        assert_eq!(config.packages[1].provider, "brew");
        assert_eq!(config.packages[1].name, "sketchybar");
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
