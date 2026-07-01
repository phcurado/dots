mod apply;
mod command;
mod config;
mod font;
mod output;
mod package;
mod plan;
mod platform;
mod project;
mod service;
mod state;
mod symlink;
mod user;

use anyhow::{Context, Result, bail};
use apply::apply_plan;
use clap::{Parser, Subcommand};
use config::load_config;
use output::{
    bold, print_plan, print_state, print_state_initialized, summarize_plan, with_spinner,
};
use plan::{build_plan, refresh_state_from_system};
use platform::selected_profile;
use project::{Project, find_project};
use state::{State, load_state, save_state};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use symlink::expand_home;

#[derive(Debug, Parser)]
#[command(name = "dots", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Lua config file. Defaults to dots.lua in the project root.
    #[arg(short, long, global = true)]
    file: Option<PathBuf>,

    /// Profile name for host or machine-specific config.
    #[arg(long, global = true)]
    profile: Option<String>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Check what would change without applying it.
    #[command(visible_alias = "plan")]
    Check,
    /// Create a starter dots.lua and ignore local state.
    Init,
    /// Apply the checked changes.
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

fn main() -> Result<()> {
    let cli = Cli::parse();
    let command = cli.command.unwrap_or(Command::Check);

    if matches!(command, Command::Init) {
        return init_project(cli.file.as_deref());
    }

    let profile = selected_profile(cli.profile.as_deref())?;
    let project = find_project(cli.file).map_err(|error| {
        if error.to_string() == "could not find dots.lua" {
            anyhow::anyhow!("No dots project found.\n\nRun `dots init` to start.")
        } else {
            error
        }
    })?;
    let state_path = project.root.join(".dots/state.json");
    let state_exists = state_path.exists();
    let mut state = load_state(&state_path)?;

    match command {
        Command::Check => {
            if !state_exists {
                print_state_initialized(&project, &state_path);
            }
            let plan = with_spinner("Checking system...", || {
                let config = load_config(&project, &profile)?;
                refresh_state_from_system(&config, &mut state)?;
                save_state(&state_path, &state)?;
                build_plan(&config, &state)
            })?;
            print_plan(&project, &plan, true);
        }
        Command::Apply => {
            if !state_exists {
                print_state_initialized(&project, &state_path);
            }
            let plan = with_spinner("Checking system...", || {
                let config = load_config(&project, &profile)?;
                refresh_state_from_system(&config, &mut state)?;
                save_state(&state_path, &state)?;
                build_plan(&config, &state)
            })?;
            print_plan(&project, &plan, false);
            confirm_apply(&plan)?;
            apply_plan(&plan, &mut state)?;
            save_state(&state_path, &state)?;
        }
        Command::State { command } => {
            run_state_command(&project, &state_path, &mut state, command)?;
        }
        Command::Init => unreachable!(),
    }

    Ok(())
}

fn init_project(file: Option<&Path>) -> Result<()> {
    let config = match file {
        Some(path) => path.to_path_buf(),
        None => std::env::current_dir()?.join("dots.lua"),
    };
    let root = config.parent().context("config path has no parent")?;
    let mut changed = false;
    let mut created_config = false;

    if !config.exists() {
        fs::write(&config, starter_config())?;
        changed = true;
        created_config = true;
    }

    let gitignore = root.join(".gitignore");
    let mut ignored = fs::read_to_string(&gitignore).unwrap_or_default();
    if !ignored.lines().any(|line| line.trim() == ".dots/") {
        if !ignored.is_empty() && !ignored.ends_with('\n') {
            ignored.push('\n');
        }
        ignored.push_str(".dots/\n");
        fs::write(&gitignore, ignored)?;
        changed = true;
    }

    if created_config {
        println!("{}", bold("Initialized dots project."));
        println!();
        println!("See dots.lua for examples. When you add something, run `dots check`.");
    } else if changed {
        println!("{}", bold("Initialized dots project."));
    } else {
        println!("{}", bold("Already initialized."));
    }

    Ok(())
}

fn starter_config() -> &'static str {
    r#"-- dots.lua
-- Docs: https://phcurado.github.io/dots/

local packages = { "bat", "ripgrep" }

---- Files
-- The source path is relative to this repo and must exist.
-- dots.symlink("~/.zshrc", ".zshrc")
-- dots.fonts.install()

---- Arch Linux
-- if dots.platform.family == "arch" then
-- 	dots.pacman.install({ "base-devel", "git" })
--
-- 	-- AUR helper: yay.
-- 	dots.yay.enable({ method = "aur" })
-- 	dots.yay.install(packages)
--
-- 	-- Alternative: paru.
-- 	-- dots.paru.enable({ method = "pacman" })
-- 	-- dots.paru.install(packages)
--
-- 	dots.systemd.enable({ "docker.service" })
-- 	dots.systemd.start({ "docker.service" })
-- end

---- Debian / Ubuntu
-- if dots.platform.family == "debian" then
-- 	dots.apt.install(packages)
-- end

---- macOS
-- if dots.platform.family == "darwin" then
-- 	dots.brew.enable()
-- 	dots.brew.install(packages)
-- 	dots.brew.cask({ "firefox" })
-- 	dots.brew.service.start({ "sketchybar" })
-- end

---- Profiles
-- if dots.profile == "work" then
-- 	dots.symlink("~/.gitconfig", "profiles/work/gitconfig")
-- end
"#
}

fn confirm_apply(plan: &[plan::PlanStep]) -> Result<()> {
    let summary = summarize_plan(plan);
    if summary.conflicts > 0 || summary.create + summary.update + summary.remove == 0 {
        return Ok(());
    }

    println!();
    println!("Type 'yes' to apply these changes.");
    print!("Apply? ");
    io::stdout().flush()?;

    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    if answer.trim() != "yes" {
        bail!("apply cancelled");
    }
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
                println!("{} {}", bold("Forgot:"), resource);
            } else {
                bail!("resource is not tracked: {resource}");
            }
        }
    }
    Ok(())
}

fn state_key_from_arg(resource: &str) -> String {
    if resource.starts_with("symlink:")
        || resource.starts_with("package:")
        || resource.starts_with("service:")
        || resource.starts_with("font:")
        || resource.starts_with("group:")
        || resource.starts_with("user-group:")
    {
        resource.to_string()
    } else {
        format!("symlink:{}", expand_home(resource).display())
    }
}
