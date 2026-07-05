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
    bold, print_plan, print_state, print_state_initialized, print_symlink_candidates,
    summarize_plan, with_spinner,
};
use plan::{PlanStep, build_plan, refresh_state_from_system};
use platform::selected_profile;
use project::{Project, find_project};
use state::{State, StateResource, load_state, save_state};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use symlink::{
    SymlinkCandidate, apply_symlink_candidate, expand_home, symlink_candidate_for_resource,
    symlink_candidate_for_target,
};

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
    Apply {
        /// Apply without prompting for confirmation.
        #[arg(long)]
        auto_approve: bool,
    },
    /// Import existing target files into the repo and link them back.
    Symlink {
        /// Existing target path to import.
        path: Option<String>,
        #[command(subcommand)]
        command: Option<SymlinkCommand>,
    },
    /// Inspect or edit local state.
    State {
        #[command(subcommand)]
        command: StateCommand,
    },
}

#[derive(Debug, Subcommand)]
enum SymlinkCommand {
    /// Import and link unmanaged symlink candidates.
    Apply {
        /// Existing target path to import.
        path: Option<String>,
        /// Apply without prompting for confirmation.
        #[arg(long)]
        auto_approve: bool,
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
            let plan = check_project(&project, &profile, &state_path, state_exists, &mut state)?;
            print_plan(&project, &plan, true);
        }
        Command::Apply { auto_approve } => {
            let plan = check_project(&project, &profile, &state_path, state_exists, &mut state)?;
            print_plan(&project, &plan, false);
            confirm_apply(&plan, auto_approve)?;
            apply_plan(&plan, &mut state)?;
            save_state(&state_path, &state)?;
        }
        Command::Symlink { path, command } => {
            let plan = check_project(&project, &profile, &state_path, state_exists, &mut state)?;
            run_symlink_command(
                &project,
                &profile,
                &state_path,
                &mut state,
                &plan,
                path.as_deref(),
                command,
            )?;
        }
        Command::State { command } => {
            run_state_command(&project, &state_path, &mut state, command)?;
        }
        Command::Init => unreachable!(),
    }

    Ok(())
}

fn check_project(
    project: &Project,
    profile: &str,
    state_path: &Path,
    state_exists: bool,
    state: &mut State,
) -> Result<Vec<plan::PlanStep>> {
    if !state_exists {
        print_state_initialized(project, state_path);
    }
    with_spinner("Checking system...", || {
        let config = load_config(project, profile)?;
        refresh_state_from_system(&config, state)?;
        save_state(state_path, state)?;
        build_plan(&config, state)
    })
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
-- Use `dots symlink` to import existing target files into this repo.
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

fn confirm_apply(plan: &[plan::PlanStep], auto_approve: bool) -> Result<()> {
    let summary = summarize_plan(plan);
    if auto_approve || summary.conflicts > 0 || summary.total_changes() == 0 {
        return Ok(());
    }

    println!();
    if summary.symlink_candidates > 0 {
        println!("This will import files into the repo and link them back.");
    }
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

fn run_symlink_command(
    project: &Project,
    profile: &str,
    state_path: &Path,
    state: &mut State,
    plan: &[PlanStep],
    path: Option<&str>,
    command: Option<SymlinkCommand>,
) -> Result<()> {
    match command {
        None => {
            if let Some(path) = path {
                let candidates = symlink_candidates(project, profile, plan, Some(path))?;
                print_symlink_candidates(project, candidates.iter());
                println!();
                println!(
                    "{}",
                    output::dim(&format!(
                        "Run `dots symlink apply {}` to apply.",
                        shell_arg(path)
                    ))
                );
                return Ok(());
            }

            let steps = symlink_review_steps(plan);
            if steps.is_empty() {
                println!("No symlink changes.");
                println!(
                    "{}",
                    output::dim(
                        "To import a file under a declared directory, pass its path: `dots symlink ~/.config/app/file`."
                    )
                );
                return Ok(());
            }

            print_plan(project, &steps, false);
            let summary = summarize_plan(&steps);
            if summary.conflicts == 0 && summary.total_changes() > 0 {
                println!("{}", output::dim("Run `dots symlink apply` to apply."));
            }
            Ok(())
        }
        Some(SymlinkCommand::Apply {
            path: apply_path,
            auto_approve,
        }) => {
            if apply_path.is_some() || path.is_some() {
                let candidates =
                    symlink_candidates(project, profile, plan, apply_path.as_deref().or(path))?;
                return apply_symlink_candidates(
                    project,
                    state_path,
                    state,
                    candidates,
                    auto_approve,
                );
            }

            apply_symlink_plan(project, state_path, state, plan, auto_approve)
        }
    }
}

fn symlink_review_steps(plan: &[PlanStep]) -> Vec<PlanStep> {
    plan.iter()
        .filter(|step| {
            matches!(
                step,
                PlanStep::SymlinkCreate(_)
                    | PlanStep::SymlinkUpdate(_)
                    | PlanStep::SymlinkRemove { .. }
                    | PlanStep::SymlinkConflict { .. }
                    | PlanStep::SymlinkCandidate(_)
            )
        })
        .cloned()
        .collect()
}

fn symlink_apply_steps(plan: &[PlanStep]) -> Vec<PlanStep> {
    plan.iter()
        .filter(|step| {
            matches!(
                step,
                PlanStep::SymlinkCreate(_)
                    | PlanStep::SymlinkUpdate(_)
                    | PlanStep::SymlinkRemove { .. }
                    | PlanStep::SymlinkCandidate(_)
                    | PlanStep::SymlinkConflict { .. }
            )
        })
        .cloned()
        .collect()
}

fn symlink_candidates(
    project: &Project,
    profile: &str,
    plan: &[PlanStep],
    path: Option<&str>,
) -> Result<Vec<SymlinkCandidate>> {
    if let Some(path) = path {
        return Ok(vec![symlink_candidate_for_path(project, profile, path)?]);
    }

    Ok(plan
        .iter()
        .filter_map(|step| match step {
            PlanStep::SymlinkCandidate(candidate) => Some(candidate.clone()),
            _ => None,
        })
        .collect())
}

fn symlink_candidate_for_path(
    project: &Project,
    profile: &str,
    path: &str,
) -> Result<SymlinkCandidate> {
    let target = expand_home(path);
    let config = load_config(project, profile)?;

    for resource in &config.symlinks {
        if resource.target == target {
            if let Some(candidate) = symlink_candidate_for_resource(resource)? {
                return Ok(candidate);
            }
        }
    }

    for declaration in &config.symlink_declarations {
        if let Some(candidate) = symlink_candidate_for_target(declaration, &target)? {
            return Ok(candidate);
        }
    }

    bail!(
        "no unmanaged symlink candidate for {}",
        output::display_target(&target)
    )
}

fn apply_symlink_plan(
    project: &Project,
    state_path: &Path,
    state: &mut State,
    plan: &[PlanStep],
    auto_approve: bool,
) -> Result<()> {
    let apply_steps = symlink_apply_steps(plan);
    let review_steps = symlink_review_steps(plan);
    if review_steps.is_empty() {
        println!("No symlink changes.");
        println!(
            "{}",
            output::dim(
                "To import a file under a declared directory, pass its path: `dots symlink apply ~/.config/app/file`."
            )
        );
        return Ok(());
    }

    print_plan(project, &review_steps, false);
    let summary = summarize_plan(&review_steps);
    if summary.conflicts > 0 {
        bail!(
            "symlink plan has {} conflict(s); refusing to apply",
            summary.conflicts
        );
    }
    confirm_symlink_apply(summary.total_changes(), auto_approve)?;

    apply_plan(&apply_steps, state)?;
    save_state(state_path, state)?;

    Ok(())
}

fn apply_symlink_candidates(
    project: &Project,
    state_path: &Path,
    state: &mut State,
    candidates: Vec<SymlinkCandidate>,
    auto_approve: bool,
) -> Result<()> {
    if candidates.is_empty() {
        println!("No unmanaged symlink candidates.");
        return Ok(());
    }

    print_symlink_candidates(project, candidates.iter());
    let count = candidates.len();
    confirm_symlink_apply(count, auto_approve)?;
    println!();
    println!("{}", bold("Symlinking:"));
    for candidate in &candidates {
        output::apply_with_status(
            "Importing",
            "Import",
            &format!("symlink.{}", output::display_target(&candidate.target)),
            || apply_symlink_candidate(candidate, state),
        )?;
    }
    save_state(state_path, state)?;
    println!();
    println!("{} {} imported.", bold("Symlink complete:"), count);

    Ok(())
}

fn shell_arg(value: &str) -> String {
    if value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "_+-=./~:".contains(character))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn confirm_symlink_apply(count: usize, auto_approve: bool) -> Result<()> {
    if auto_approve || count == 0 {
        return Ok(());
    }

    println!();
    println!("Type 'yes' to import and link these files.");
    print!("Symlink? ");
    io::stdout().flush()?;

    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    if answer.trim() != "yes" {
        bail!("symlink cancelled");
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
    if StateResource::KEY_PREFIXES
        .iter()
        .any(|prefix| resource.starts_with(prefix))
    {
        resource.to_string()
    } else {
        format!("symlink:{}", expand_home(resource).display())
    }
}
