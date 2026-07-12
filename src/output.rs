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
    pub(crate) forget: usize,
    pub(crate) output_changes: usize,
}

impl PlanSummary {
    pub(crate) fn total_changes(&self) -> usize {
        self.create
            + self.update
            + self.remove
            + self.symlink_candidates
            + self.forget
            + self.output_changes
    }
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

fn display_output_value(value: Option<&serde_json::Value>) -> String {
    value
        .map(|value| serde_json::to_string(value).expect("JSON output value"))
        .unwrap_or_else(|| "known after apply".to_string())
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
        SymlinkConflictReason::TargetExistsDifferentContent => {
            "target exists with different contents".to_string()
        }
        SymlinkConflictReason::TargetExistsNotSymlink => {
            "target exists and is not a symlink".to_string()
        }
    }
}

pub(crate) fn print_symlink_candidates<'a>(
    project: &Project,
    candidates: impl IntoIterator<Item = &'a SymlinkCandidate>,
) {
    println!("{}", bold("Symlinks:"));
    for candidate in candidates {
        println!(
            "  {} import {} -> {}",
            green("+"),
            display_target(&candidate.target),
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
            | PlanStep::SystemdUnitCreate(_)
            | PlanStep::ComposeCreate(_)
            | PlanStep::FontCreate(_)
            | PlanStep::FileCreate(_)
            | PlanStep::SshKeypairCreate(_)
            | PlanStep::SystemGroupCreate(_)
            | PlanStep::UserGroupAdd(_)
            | PlanStep::CommandCreate(_) => summary.create += 1,
            PlanStep::SymlinkUpdate(_)
            | PlanStep::SystemdUnitUpdate(_)
            | PlanStep::ComposeUpdate(_)
            | PlanStep::FontUpdate(_)
            | PlanStep::FileUpdate(_)
            | PlanStep::FileModeUpdate(_)
            | PlanStep::SshKeypairAdopt(_)
            | PlanStep::SshKeypairPermissionUpdate { .. }
            | PlanStep::UserShellUpdate { .. } => summary.update += 1,
            PlanStep::SymlinkRemove { .. }
            | PlanStep::PackageRemove { .. }
            | PlanStep::ServiceRemove { .. }
            | PlanStep::SystemdUnitRemove(_)
            | PlanStep::ComposeRemove { .. }
            | PlanStep::FontRemove { .. }
            | PlanStep::SystemGroupRemove(_)
            | PlanStep::UserGroupRemove(_) => summary.remove += 1,
            PlanStep::SymlinkConflict { .. }
            | PlanStep::PackageConflict { .. }
            | PlanStep::ServiceConflict { .. }
            | PlanStep::SystemdUnitConflict { .. }
            | PlanStep::ComposeConflict { .. }
            | PlanStep::FontConflict { .. }
            | PlanStep::FileConflict { .. }
            | PlanStep::SshKeypairConflict { .. }
            | PlanStep::SystemGroupConflict { .. }
            | PlanStep::UserGroupConflict { .. }
            | PlanStep::CapabilityConflict { .. } => summary.conflicts += 1,
            PlanStep::SymlinkCandidate(_) => summary.symlink_candidates += 1,
            PlanStep::FileForget { .. } | PlanStep::SshKeypairForget { .. } => summary.forget += 1,
            PlanStep::OutputCreate { .. }
            | PlanStep::OutputUpdate { .. }
            | PlanStep::OutputRemove { .. } => summary.output_changes += 1,
            PlanStep::SymlinkNoop(_)
            | PlanStep::PackageNoop { .. }
            | PlanStep::ServiceNoop(_)
            | PlanStep::SystemdUnitNoop(_)
            | PlanStep::ComposeNoop { .. }
            | PlanStep::FontNoop(_)
            | PlanStep::FileNoop(_)
            | PlanStep::SshKeypairNoop { .. }
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
    let has_changes = summary.total_changes() + summary.conflicts > 0;
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
                | PlanStep::SymlinkCandidate(_)
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
                PlanStep::SymlinkCandidate(candidate) => println!(
                    "  {} import {} -> {}",
                    green("+"),
                    display_target(&candidate.target),
                    display_source(project, &candidate.source),
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
        if has_capabilities || has_symlinks {
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
        if has_capabilities || has_symlinks || has_packages {
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

    let has_ssh_keypairs = plan.iter().any(|step| {
        matches!(
            step,
            PlanStep::SshKeypairCreate(_)
                | PlanStep::SshKeypairAdopt(_)
                | PlanStep::SshKeypairPermissionUpdate { .. }
                | PlanStep::SshKeypairForget { .. }
                | PlanStep::SshKeypairConflict { .. }
        )
    });
    if has_ssh_keypairs {
        if has_capabilities || has_symlinks || has_packages || has_fonts {
            println!();
        }
        println!("{}", bold("SSH keypairs:"));
        for step in plan {
            match step {
                PlanStep::SshKeypairCreate(resource) => println!(
                    "  {} {} {}",
                    green("+"),
                    resource.name,
                    display_target(&resource.private_path)
                ),
                PlanStep::SshKeypairAdopt(resource) => println!(
                    "  {} {} {} (passphrase validation required)",
                    yellow("~"),
                    resource.name,
                    display_target(&resource.private_path)
                ),
                PlanStep::SshKeypairPermissionUpdate { resource, .. } => println!(
                    "  {} {} {} (permissions)",
                    yellow("~"),
                    resource.name,
                    display_target(&resource.private_path)
                ),
                PlanStep::SshKeypairForget { name } => {
                    println!("  {} forget {name}", yellow("~"))
                }
                PlanStep::SshKeypairConflict { resource, reason } => println!(
                    "  {} {} {} ({reason})",
                    red("!"),
                    resource.name,
                    display_target(&resource.private_path)
                ),
                _ => {}
            }
        }
    }

    let has_files = plan.iter().any(|step| {
        matches!(
            step,
            PlanStep::FileCreate(_)
                | PlanStep::FileUpdate(_)
                | PlanStep::FileModeUpdate(_)
                | PlanStep::FileForget { .. }
                | PlanStep::FileConflict { .. }
        )
    });
    if has_files {
        if has_capabilities || has_symlinks || has_packages || has_fonts || has_ssh_keypairs {
            println!();
        }
        println!("{}", bold("Files:"));
        for step in plan {
            match step {
                PlanStep::FileCreate(resource) => {
                    println!("  {} {}", green("+"), display_target(&resource.target))
                }
                PlanStep::FileUpdate(resource) | PlanStep::FileModeUpdate(resource) => {
                    println!("  {} {}", yellow("~"), display_target(&resource.target))
                }
                PlanStep::FileForget { target } => {
                    println!("  {} forget {}", yellow("~"), display_target(target))
                }
                PlanStep::FileConflict { resource, reason } => println!(
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
        if has_capabilities
            || has_symlinks
            || has_packages
            || has_fonts
            || has_ssh_keypairs
            || has_files
        {
            println!();
        }
        println!("{}", bold("Commands:"));
        for step in plan {
            if let PlanStep::CommandCreate(resource) = step {
                println!("  {} {}", green("+"), resource.name);
            }
        }
    }

    let has_systemd_units = plan.iter().any(|step| {
        matches!(
            step,
            PlanStep::SystemdUnitCreate(_)
                | PlanStep::SystemdUnitUpdate(_)
                | PlanStep::SystemdUnitRemove(_)
                | PlanStep::SystemdUnitConflict { .. }
        )
    });
    if has_systemd_units {
        if has_capabilities
            || has_symlinks
            || has_packages
            || has_fonts
            || has_ssh_keypairs
            || has_files
            || has_commands
        {
            println!();
        }
        println!("{}", bold("Systemd:"));
        for step in plan {
            match step {
                PlanStep::SystemdUnitCreate(resource) => {
                    println!("  {} apply {}", green("+"), resource.unit)
                }
                PlanStep::SystemdUnitUpdate(resource) => {
                    println!("  {} apply {}", yellow("~"), resource.unit)
                }
                PlanStep::SystemdUnitRemove(resource) => {
                    println!("  {} remove {}", red("-"), resource.unit)
                }
                PlanStep::SystemdUnitConflict { resource, reason } => {
                    println!("  {} {} ({reason})", red("!"), resource.unit)
                }
                _ => {}
            }
        }
    }

    let has_compose = plan.iter().any(|step| {
        matches!(
            step,
            PlanStep::ComposeCreate(_)
                | PlanStep::ComposeUpdate(_)
                | PlanStep::ComposeRemove { .. }
                | PlanStep::ComposeConflict { .. }
        )
    });
    if has_compose {
        if has_capabilities
            || has_symlinks
            || has_packages
            || has_fonts
            || has_ssh_keypairs
            || has_files
            || has_commands
            || has_systemd_units
        {
            println!();
        }
        println!("{}", bold("Docker Compose:"));
        for step in plan {
            match step {
                PlanStep::ComposeCreate(resource) => {
                    println!("  {} apply {}", green("+"), resource.name)
                }
                PlanStep::ComposeUpdate(resource) => {
                    println!("  {} apply {}", yellow("~"), resource.name)
                }
                PlanStep::ComposeRemove { resource, .. } => {
                    println!("  {} remove {}", red("-"), resource.name)
                }
                PlanStep::ComposeConflict { resource, reason } => {
                    println!("  {} {} ({reason})", red("!"), resource.name)
                }
                _ => {}
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
            || has_packages
            || has_fonts
            || has_ssh_keypairs
            || has_files
            || has_commands
            || has_compose
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
            || has_packages
            || has_fonts
            || has_ssh_keypairs
            || has_files
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
            || has_packages
            || has_fonts
            || has_ssh_keypairs
            || has_files
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

    let has_outputs = plan.iter().any(|step| {
        matches!(
            step,
            PlanStep::OutputCreate { .. }
                | PlanStep::OutputUpdate { .. }
                | PlanStep::OutputRemove { .. }
        )
    });
    if has_outputs {
        println!();
        println!("{}", bold("Outputs:"));
        for step in plan {
            match step {
                PlanStep::OutputCreate { name, value } => println!(
                    "  {} {name} = {}",
                    green("+"),
                    display_output_value(value.as_ref())
                ),
                PlanStep::OutputUpdate {
                    name,
                    before,
                    after,
                } => println!(
                    "  {} {name}: {} → {}",
                    yellow("~"),
                    serde_json::to_string(before).expect("JSON output value"),
                    display_output_value(after.as_ref())
                ),
                PlanStep::OutputRemove { name, value } => println!(
                    "  {} {name} = {}",
                    red("-"),
                    serde_json::to_string(value).expect("JSON output value")
                ),
                _ => {}
            }
        }
    }

    println!();
    let import_text = if summary.symlink_candidates > 0 {
        format!(
            "{} to import, ",
            green(&summary.symlink_candidates.to_string())
        )
    } else {
        String::new()
    };
    let forget_text = if summary.forget > 0 {
        format!("{} to forget, ", yellow(&summary.forget.to_string()))
    } else {
        String::new()
    };
    let output_text = if summary.output_changes > 0 {
        format!(
            "{} {}, ",
            yellow(&summary.output_changes.to_string()),
            if summary.output_changes == 1 {
                "output change"
            } else {
                "output changes"
            }
        )
    } else {
        String::new()
    };
    println!(
        "{} {import_text}{forget_text}{output_text}{} to create, {} to update, {} to destroy{}",
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

    if show_apply_hint && summary.conflicts == 0 && summary.total_changes() > 0 {
        println!("{}", dim("Run `dots apply` to apply these changes."));
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
            StateResource::SystemdUnit { unit, file, .. } => {
                println!("  systemd {unit} {}", display_source(project, file))
            }
            StateResource::Compose { name, file, .. } => {
                println!("  docker compose {name} {}", display_source(project, file))
            }
            StateResource::Font { target, .. } => println!("  font {}", display_target(target)),
            StateResource::File { target, .. } => println!("  file {}", display_target(target)),
            StateResource::SshKeypair {
                name, private_path, ..
            } => println!("  SSH keypair {name} {}", display_target(private_path)),
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
