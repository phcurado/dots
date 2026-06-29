mod apply;
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

use anyhow::{Result, bail};
use apply::apply_plan;
use clap::{Parser, Subcommand};
use config::load_config;
use output::{bold, print_plan, print_state, print_state_initialized};
use plan::{build_plan, refresh_state_from_system};
use platform::selected_profile;
use project::{Project, find_project};
use state::{State, load_state, save_state};
use std::path::{Path, PathBuf};
use symlink::expand_home;

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
    {
        resource.to_string()
    } else {
        format!("symlink:{}", expand_home(resource).display())
    }
}
