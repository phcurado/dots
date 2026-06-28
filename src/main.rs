use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use mlua::{Lua, Table, Value};
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
#[command(name = "dots", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Lua config file. Defaults to dots.lua or dots/init.lua in the project root.
    #[arg(short, long, global = true)]
    file: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show the planned changes without applying them.
    Plan,
    /// Apply the planned changes.
    Apply,
}

#[derive(Debug, Clone)]
struct SymlinkResource {
    target: PathBuf,
    source: PathBuf,
}

#[derive(Debug, Clone)]
enum PlanStep {
    Create(SymlinkResource),
    Update(SymlinkResource),
    Remove(StateResource),
    Noop(SymlinkResource),
    Conflict {
        resource: SymlinkResource,
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StateResource {
    kind: String,
    target: PathBuf,
    source: PathBuf,
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
    let project = find_project(cli.file)?;
    let resources = load_config(&project)?;
    let state_path = project.root.join(".dots/state.json");
    let mut state = load_state(&state_path)?;
    let plan = build_plan(&resources, &state)?;

    print_plan(&plan);

    if matches!(cli.command, Command::Apply) {
        apply_plan(&plan, &mut state)?;
        save_state(&state_path, &state)?;
    }

    Ok(())
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
        let dots_lua = dir.join("dots.lua");
        if dots_lua.exists() {
            return Ok(Project {
                root: dir,
                config: dots_lua,
            });
        }

        let init_lua = dir.join("dots/init.lua");
        if init_lua.exists() {
            return Ok(Project {
                root: dir,
                config: init_lua,
            });
        }

        if !dir.pop() {
            bail!("could not find dots.lua or dots/init.lua")
        }
    }
}

fn load_config(project: &Project) -> Result<Vec<SymlinkResource>> {
    let lua = Lua::new();
    let resources = lua.create_table()?;
    let dots = lua.create_table()?;

    let root = project.root.clone();
    let symlink = lua.create_function(move |_, args: mlua::MultiValue| {
        let values = args.into_iter().collect::<Vec<_>>();
        if values.len() != 2 {
            return Err(mlua::Error::RuntimeError(
                "expected dots.symlink(target, source)".to_string(),
            ));
        }

        let target = value_to_string(&values[0], "target")?;
        let source = value_to_string(&values[1], "source")?;

        let item = lua.create_table()?;
        item.set("type", "symlink")?;
        item.set("target", expand_home(&target).display().to_string())?;
        item.set(
            "source",
            resolve_source(&root, &source).display().to_string(),
        )?;
        resources.raw_push(item)?;
        Ok(())
    })?;

    dots.set("symlink", symlink)?;
    dots.set("root", project.root.display().to_string())?;

    let package: Table = lua.globals().get("package")?;
    let preload: Table = package.get("preload")?;
    let dots_module = dots.clone();
    preload.set(
        "dots",
        lua.create_function(move |_, ()| Ok(dots_module.clone()))?,
    )?;
    lua.globals().set("dots", dots)?;

    install_package_path(&lua, &project.root)?;

    let source = fs::read_to_string(&project.config)
        .with_context(|| format!("failed to read {}", project.config.display()))?;
    lua.load(&source)
        .set_name(project.config.display().to_string())
        .exec()?;

    let mut out = Vec::new();
    for item in resources.sequence_values::<Table>() {
        let item = item?;
        out.push(SymlinkResource {
            target: PathBuf::from(item.get::<String>("target")?),
            source: PathBuf::from(item.get::<String>("source")?),
        });
    }
    Ok(out)
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

fn build_plan(resources: &[SymlinkResource], state: &State) -> Result<Vec<PlanStep>> {
    let mut plan = Vec::new();
    let mut declared = BTreeSet::new();

    for resource in resources {
        let id = resource_id(resource);
        declared.insert(id.clone());
        let owned = state.resources.contains_key(&id);

        if !resource.source.exists() {
            plan.push(PlanStep::Conflict {
                resource: resource.clone(),
                reason: format!("source does not exist: {}", resource.source.display()),
            });
            continue;
        }

        match fs::symlink_metadata(&resource.target) {
            Ok(meta) if meta.file_type().is_symlink() => {
                let current = fs::read_link(&resource.target)?;
                if same_path(&current, &resource.source) {
                    plan.push(PlanStep::Noop(resource.clone()));
                } else if owned {
                    plan.push(PlanStep::Update(resource.clone()));
                } else {
                    plan.push(PlanStep::Conflict {
                        resource: resource.clone(),
                        reason: format!("target is unmanaged symlink to {}", current.display()),
                    });
                }
            }
            Ok(_) => plan.push(PlanStep::Conflict {
                resource: resource.clone(),
                reason: "target exists and is not a symlink".to_string(),
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                plan.push(PlanStep::Create(resource.clone()));
            }
            Err(error) => return Err(error.into()),
        }
    }

    for (id, resource) in &state.resources {
        if resource.kind == "symlink" && !declared.contains(id) {
            plan.push(PlanStep::Remove(resource.clone()));
        }
    }

    Ok(plan)
}

fn same_path(left: &Path, right: &Path) -> bool {
    left == right || fs::canonicalize(left).ok() == fs::canonicalize(right).ok()
}

fn resource_id(resource: &SymlinkResource) -> String {
    format!("symlink:{}", resource.target.display())
}

fn print_plan(plan: &[PlanStep]) {
    println!("Symlinks:");
    if plan.is_empty() {
        println!("  no changes");
        return;
    }

    for step in plan {
        match step {
            PlanStep::Create(resource) => println!(
                "  + {} -> {}",
                resource.target.display(),
                resource.source.display()
            ),
            PlanStep::Update(resource) => println!(
                "  ~ {} -> {}",
                resource.target.display(),
                resource.source.display()
            ),
            PlanStep::Remove(resource) => println!("  - {}", resource.target.display()),
            PlanStep::Noop(resource) => println!("  = {}", resource.target.display()),
            PlanStep::Conflict { resource, reason } => {
                println!("  ! {} ({reason})", resource.target.display())
            }
        }
    }
}

fn apply_plan(plan: &[PlanStep], state: &mut State) -> Result<()> {
    let conflicts = plan
        .iter()
        .filter(|step| matches!(step, PlanStep::Conflict { .. }))
        .count();
    if conflicts > 0 {
        bail!("plan has {conflicts} conflict(s); refusing to apply")
    }

    for step in plan {
        match step {
            PlanStep::Create(resource) | PlanStep::Update(resource) => {
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
                state.resources.insert(
                    resource_id(resource),
                    StateResource {
                        kind: "symlink".to_string(),
                        target: resource.target.clone(),
                        source: resource.source.clone(),
                    },
                );
            }
            PlanStep::Remove(resource) => {
                if fs::symlink_metadata(&resource.target)
                    .map(|meta| meta.file_type().is_symlink())
                    .unwrap_or(false)
                {
                    let current = fs::read_link(&resource.target)?;
                    if same_path(&current, &resource.source) {
                        fs::remove_file(&resource.target)?;
                    }
                }
                state
                    .resources
                    .remove(&format!("symlink:{}", resource.target.display()));
            }
            PlanStep::Noop(resource) => {
                state.resources.insert(
                    resource_id(resource),
                    StateResource {
                        kind: "symlink".to_string(),
                        target: resource.target.clone(),
                        source: resource.source.clone(),
                    },
                );
            }
            PlanStep::Conflict { .. } => unreachable!(),
        }
    }

    Ok(())
}
