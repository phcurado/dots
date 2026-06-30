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
    let default_check = cli.command.is_none();
    let command = cli.command.unwrap_or(Command::Check);

    if matches!(command, Command::Init) {
        return init_project(cli.file.as_deref());
    }

    let profile = selected_profile(cli.profile.as_deref())?;
    let project = if default_check && cli.file.is_none() {
        find_project_or_offer_init()?
    } else {
        find_project(cli.file)?
    };
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
            print_plan(&project, &plan);
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
            print_plan(&project, &plan);
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

fn find_project_or_offer_init() -> Result<Project> {
    match find_project(None) {
        Ok(project) => Ok(project),
        Err(error) if error.to_string() == "could not find dots.lua" => {
            if !confirm_init()? {
                return Err(error);
            }
            init_project(None)?;
            find_project(None)
        }
        Err(error) => Err(error),
    }
}

fn confirm_init() -> Result<bool> {
    println!("No dots project found here or in any parent directory.");
    print!("Create a dots project in this folder? [y/N] ");
    io::stdout().flush()?;

    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "YES"))
}

fn init_project(file: Option<&Path>) -> Result<()> {
    let config = match file {
        Some(path) => path.to_path_buf(),
        None => std::env::current_dir()?.join("dots.lua"),
    };
    let root = config.parent().context("config path has no parent")?;
    let mut changed = false;

    if !config.exists() {
        fs::write(&config, starter_config())?;
        println!("{} {}", bold("Created:"), config.display());
        changed = true;
    }

    let gitignore = root.join(".gitignore");
    let mut ignored = fs::read_to_string(&gitignore).unwrap_or_default();
    if !ignored.lines().any(|line| line.trim() == ".dots/") {
        if !ignored.is_empty() && !ignored.ends_with('\n') {
            ignored.push('\n');
        }
        ignored.push_str(".dots/\n");
        fs::write(&gitignore, ignored)?;
        println!("{} {}", bold("Updated:"), gitignore.display());
        changed = true;
    }

    if !changed {
        println!("{}", bold("Already initialized."));
    }

    Ok(())
}

fn starter_config() -> &'static str {
    r#"-- Start with one file, run `dots check`, then add more.
-- dots.symlink("~/.zshrc", ".zshrc")

-- if dots.platform.family == "arch" then
-- 	dots.pacman.install({ "base-devel", "git", "paru" })
-- 	dots.paru.install({ "bat", "ripgrep" })
-- end

-- if dots.platform.family == "darwin" then
-- 	dots.brew.install({ "bat", "ripgrep" })
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
    {
        resource.to_string()
    } else {
        format!("symlink:{}", expand_home(resource).display())
    }
}
