use std::io::{IsTerminal, Write};
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;

use anyhow::Result;
use owo_colors::OwoColorize;

use crate::plan::{PlanStep, SymlinkConflictReason};
use crate::project::Project;
use crate::state::{State, StateResource};
use crate::symlink::{SymlinkCandidate, home_dir};

#[derive(Debug, Default)]
pub(crate) struct PlanSummary {
    pub(crate) create: usize,
    pub(crate) update: usize,
    pub(crate) remove: usize,
    pub(crate) conflicts: usize,
    pub(crate) symlink_candidates: usize,
}

pub(crate) fn display_target(path: &Path) -> String {
    let home = home_dir();
    if path == home {
        return "~".to_string();
    }
    if let Ok(rest) = path.strip_prefix(&home) {
        return format!("~/{}", rest.display());
    }
    path.display().to_string()
}

pub(crate) fn display_source(project: &Project, path: &Path) -> String {
    if let Ok(rest) = path.strip_prefix(&project.root) {
        if rest.as_os_str().is_empty() {
            return ".".to_string();
        }
        return rest.display().to_string();
    }
    display_target(path)
}

fn display_symlink_conflict_reason(
    project: &Project,
    resource: &crate::symlink::SymlinkResource,
    reason: &SymlinkConflictReason,
) -> String {
    match reason {
        SymlinkConflictReason::MissingSource { current_target } => {
            let mut message = format!(
                "repo file is missing: {}",
                display_source(project, &resource.source)
            );
            if let Some(current_target) = current_target {
                message.push_str(&format!(
                    "; target is already linked to {}",
                    display_target(current_target)
                ));
            }
            message
        }
        SymlinkConflictReason::TargetUnmanaged => "target exists but is not managed".to_string(),
        SymlinkConflictReason::TargetExistsNotSymlink => {
            "target exists and is not a symlink".to_string()
        }
    }
}

pub(crate) fn print_symlink_candidates<'a>(
    project: &Project,
    candidates: impl IntoIterator<Item = &'a SymlinkCandidate>,
) {
    println!("{}", bold("Unmanaged symlink candidates:"));
    for candidate in candidates {
        println!("  {} {}", yellow("?"), display_target(&candidate.target));
        println!(
            "    can be imported to {}",
            display_source(project, &candidate.source)
        );
    }
}

pub(crate) fn summarize_plan(plan: &[PlanStep]) -> PlanSummary {
    let mut summary = PlanSummary::default();
    for step in plan {
        match step {
            PlanStep::SymlinkCreate(_)
            | PlanStep::PackageCreate { .. }
            | PlanStep::ServiceCreate { .. }
            | PlanStep::FontCreate(_)
            | PlanStep::SystemGroupCreate(_)
            | PlanStep::UserGroupAdd(_)
            | PlanStep::CommandCreate(_) => summary.create += 1,
            PlanStep::SymlinkUpdate(_)
            | PlanStep::FontUpdate(_)
            | PlanStep::UserShellUpdate { .. } => summary.update += 1,
            PlanStep::SymlinkRemove { .. }
            | PlanStep::PackageRemove { .. }
            | PlanStep::ServiceRemove { .. }
            | PlanStep::FontRemove { .. }
            | PlanStep::SystemGroupRemove(_)
            | PlanStep::UserGroupRemove(_) => summary.remove += 1,
            PlanStep::SymlinkConflict { .. }
            | PlanStep::PackageConflict { .. }
            | PlanStep::ServiceConflict { .. }
            | PlanStep::FontConflict { .. }
            | PlanStep::SystemGroupConflict { .. }
            | PlanStep::UserGroupConflict { .. }
            | PlanStep::CapabilityConflict { .. } => summary.conflicts += 1,
            PlanStep::SymlinkCandidate(_) => summary.symlink_candidates += 1,
            PlanStep::SymlinkNoop(_)
            | PlanStep::PackageNoop { .. }
            | PlanStep::ServiceNoop(_)
            | PlanStep::FontNoop(_)
            | PlanStep::UserShellNoop
            | PlanStep::SystemGroupNoop(_)
            | PlanStep::UserGroupNoop(_)
            | PlanStep::CommandNoop(_) => {}
        }
    }
    summary
}

pub(crate) fn print_plan(project: &Project, plan: &[PlanStep], show_apply_hint: bool) {
    let summary = summarize_plan(plan);
    let has_changes = summary.create
        + summary.update
        + summary.remove
        + summary.conflicts
        + summary.symlink_candidates
        > 0;
    if !has_changes {
        println!("{}", dim("No changes."));
        return;
    }

    let has_capabilities = plan
        .iter()
        .any(|step| matches!(step, PlanStep::CapabilityConflict { .. }));
    if has_capabilities {
        println!("{}", bold("Capabilities:"));
        for step in plan {
            if let PlanStep::CapabilityConflict { capability, reason } = step {
                println!("  {} {} {reason}", red("!"), capability);
            }
        }
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
                PlanStep::SymlinkRemove { target, stale, .. } => {
                    if *stale {
                        println!("  {} stale symlink {}", red("-"), display_target(target))
                    } else {
                        println!("  {} symlink {}", red("-"), display_target(target))
                    }
                }
                PlanStep::SymlinkConflict { resource, reason } => println!(
                    "  {} symlink {} ({})",
                    red("!"),
                    display_target(&resource.target),
                    display_symlink_conflict_reason(project, resource, reason),
                ),
                _ => {}
            }
        }
    }

    let has_symlink_candidates = summary.symlink_candidates > 0;
    if has_symlink_candidates {
        if has_capabilities || has_symlinks {
            println!();
        }
        print_symlink_candidates(
            project,
            plan.iter().filter_map(|step| match step {
                PlanStep::SymlinkCandidate(candidate) => Some(candidate),
                _ => None,
            }),
        );
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
        if has_capabilities || has_symlinks || has_symlink_candidates {
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

    let has_fonts = plan.iter().any(|step| {
        matches!(
            step,
            PlanStep::FontCreate(_)
                | PlanStep::FontUpdate(_)
                | PlanStep::FontRemove { .. }
                | PlanStep::FontConflict { .. }
        )
    });
    if has_fonts {
        if has_capabilities || has_symlinks || has_symlink_candidates || has_packages {
            println!();
        }
        println!("{}", bold("Fonts:"));
        for step in plan {
            match step {
                PlanStep::FontCreate(resource) => {
                    println!("  {} {}", green("+"), display_target(&resource.target))
                }
                PlanStep::FontUpdate(resource) => {
                    println!("  {} {}", yellow("~"), display_target(&resource.target))
                }
                PlanStep::FontRemove { target, .. } => {
                    println!("  {} {}", red("-"), display_target(target))
                }
                PlanStep::FontConflict { resource, reason } => println!(
                    "  {} {} ({reason})",
                    red("!"),
                    display_target(&resource.target)
                ),
                _ => {}
            }
        }
    }

    let has_commands = plan
        .iter()
        .any(|step| matches!(step, PlanStep::CommandCreate(_)));
    if has_commands {
        if has_capabilities || has_symlinks || has_symlink_candidates || has_packages || has_fonts {
            println!();
        }
        println!("{}", bold("Commands:"));
        for step in plan {
            if let PlanStep::CommandCreate(resource) = step {
                println!("  {} {}", green("+"), resource.name);
            }
        }
    }

    let has_services = plan.iter().any(|step| {
        matches!(
            step,
            PlanStep::ServiceCreate { .. }
                | PlanStep::ServiceRemove { .. }
                | PlanStep::ServiceConflict { .. }
        )
    });
    if has_services {
        if has_capabilities
            || has_symlinks
            || has_symlink_candidates
            || has_packages
            || has_fonts
            || has_commands
        {
            println!();
        }
        println!("{}", bold("Services:"));
        for step in plan {
            match step {
                PlanStep::ServiceCreate { resource, .. } => println!(
                    "  {} {} {} {}",
                    green("+"),
                    resource.provider,
                    resource.action.as_str(),
                    resource.name,
                ),
                PlanStep::ServiceRemove { resource, .. } => println!(
                    "  {} {} {} {}",
                    red("-"),
                    resource.provider,
                    resource.action.as_str(),
                    resource.name,
                ),
                PlanStep::ServiceConflict { resource, reason } => println!(
                    "  {} {} {} {} ({reason})",
                    red("!"),
                    resource.provider,
                    resource.action.as_str(),
                    resource.name,
                ),
                _ => {}
            }
        }
    }

    let has_groups = plan.iter().any(|step| {
        matches!(
            step,
            PlanStep::SystemGroupCreate(_)
                | PlanStep::SystemGroupRemove(_)
                | PlanStep::SystemGroupConflict { .. }
        )
    });
    if has_groups {
        if has_capabilities
            || has_symlinks
            || has_symlink_candidates
            || has_packages
            || has_fonts
            || has_commands
            || has_services
        {
            println!();
        }
        println!("{}", bold("Groups:"));
        for step in plan {
            match step {
                PlanStep::SystemGroupCreate(resource) => {
                    println!("  {} group {}", green("+"), resource.name)
                }
                PlanStep::SystemGroupRemove(resource) => {
                    println!("  {} group {}", red("-"), resource.name)
                }
                PlanStep::SystemGroupConflict { resource, reason } => {
                    println!("  {} group {} ({reason})", red("!"), resource.name)
                }
                _ => {}
            }
        }
    }

    let has_user = plan.iter().any(|step| {
        matches!(
            step,
            PlanStep::UserShellUpdate { .. }
                | PlanStep::UserGroupAdd(_)
                | PlanStep::UserGroupRemove(_)
                | PlanStep::UserGroupConflict { .. }
        )
    });
    if has_user {
        if has_capabilities
            || has_symlinks
            || has_symlink_candidates
            || has_packages
            || has_fonts
            || has_commands
            || has_services
            || has_groups
        {
            println!();
        }
        println!("{}", bold("User:"));
        for step in plan {
            match step {
                PlanStep::UserShellUpdate { resource, current } => println!(
                    "  {} shell {} -> {}",
                    yellow("~"),
                    current
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "unknown".to_string()),
                    resource.path.display(),
                ),
                PlanStep::UserGroupAdd(resource) => {
                    println!("  {} group {}", green("+"), resource.name)
                }
                PlanStep::UserGroupRemove(resource) => {
                    println!("  {} group {}", red("-"), resource.name)
                }
                PlanStep::UserGroupConflict { resource, reason } => {
                    println!("  {} group {} ({reason})", red("!"), resource.name)
                }
                _ => {}
            }
        }
    }

    println!();
    println!(
        "{} {} to create, {} to update, {} to destroy{}",
        bold("Check:"),
        green(&summary.create.to_string()),
        yellow(&summary.update.to_string()),
        red(&summary.remove.to_string()),
        if summary.conflicts > 0 {
            red(&format!(", {} conflicts", summary.conflicts))
        } else {
            ".".to_string()
        }
    );

    if show_apply_hint
        && summary.conflicts == 0
        && summary.create + summary.update + summary.remove > 0
    {
        println!("{}", dim("Run `dots apply` to apply these changes."));
    }
    if show_apply_hint && summary.symlink_candidates > 0 {
        println!(
            "{}",
            dim("Run `dots symlink` to review unmanaged symlink candidates.")
        );
    }
}

pub(crate) fn print_state(project: &Project, state: &State) {
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
            StateResource::Service {
                provider,
                action,
                name,
            } => println!("  service {provider} {} {name}", action.as_str()),
            StateResource::Font { target, .. } => println!("  font {}", display_target(target)),
            StateResource::Group { name } => println!("  group {name}"),
            StateResource::UserGroup { name } => println!("  user group {name}"),
        }
        println!("    {}", dim(id));
    }
}

pub(crate) fn print_state_initialized(project: &Project, state_path: &Path) {
    println!(
        "{} {}",
        dim("Created local state:"),
        dim(&display_source(project, state_path))
    );
    println!();
}

pub(crate) fn with_spinner<T>(message: &str, work: impl FnOnce() -> Result<T>) -> Result<T> {
    if !std::io::stderr().is_terminal() {
        return work();
    }

    let done = Arc::new(AtomicBool::new(false));
    let done_for_thread = done.clone();
    let message = message.to_string();
    let spinner = thread::spawn(move || {
        let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let mut index = 0;
        while !done_for_thread.load(Ordering::Relaxed) {
            eprint!("\r{} {message}", dim(frames[index % frames.len()]));
            let _ = std::io::stderr().flush();
            index += 1;
            thread::sleep(Duration::from_millis(100));
        }
        eprint!("\r\x1b[2K");
        let _ = std::io::stderr().flush();
    });

    let result = work();
    done.store(true, Ordering::Relaxed);
    let _ = spinner.join();
    result
}

pub(crate) fn apply_with_status(
    action: &str,
    noun: &str,
    id: &str,
    apply: impl FnOnce() -> Result<()>,
) -> Result<()> {
    let inline = std::io::stdout().is_terminal();
    if inline {
        print!("  {id}: {}...", dim(action));
        std::io::stdout().flush()?;
    } else {
        println!("  {id}: {}...", dim(action));
    }

    match apply() {
        Ok(()) => {
            print_apply_status(inline, id, &green(&format!("{noun} complete")));
            Ok(())
        }
        Err(error) => {
            print_apply_status(inline, id, &red(&format!("{noun} failed")));
            Err(error)
        }
    }
}

fn print_apply_status(inline: bool, id: &str, status: &str) {
    if inline {
        println!("\r\x1b[2K  {id}: {status}");
    } else {
        println!("  {id}: {status}");
    }
}

fn colors_enabled() -> bool {
    std::io::stdout().is_terminal()
}

pub(crate) fn green(value: &str) -> String {
    if colors_enabled() {
        value.green().to_string()
    } else {
        value.to_string()
    }
}

pub(crate) fn yellow(value: &str) -> String {
    if colors_enabled() {
        value.yellow().to_string()
    } else {
        value.to_string()
    }
}

pub(crate) fn red(value: &str) -> String {
    if colors_enabled() {
        value.red().to_string()
    } else {
        value.to_string()
    }
}

pub(crate) fn bold(value: &str) -> String {
    if colors_enabled() {
        value.bold().to_string()
    } else {
        value.to_string()
    }
}

pub(crate) fn dim(value: &str) -> String {
    if colors_enabled() {
        value.dimmed().to_string()
    } else {
        value.to_string()
    }
}
