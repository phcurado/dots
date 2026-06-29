use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use mlua::{Lua, Table, Value};

use crate::package::{PackageProvider, PackageResource};
use crate::plan::package_id_for;
use crate::platform::platform_table;
use crate::project::Project;
use crate::symlink::{
    SymlinkDeclaration, SymlinkResource, expand_home, expand_symlink_declaration, resolve_source,
    same_path,
};

#[derive(Debug, Default)]
pub(crate) struct Config {
    pub(crate) symlinks: Vec<SymlinkResource>,
    pub(crate) packages: Vec<PackageResource>,
    pub(crate) package_providers: BTreeMap<String, PackageProvider>,
}

pub(crate) fn load_config(project: &Project, profile: &str) -> Result<Config> {
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
