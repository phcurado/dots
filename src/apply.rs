use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::process::{Command as ProcessCommand, Stdio};

use anyhow::{Result, bail};

use crate::command::{CommandResource, command_apply, command_id_for};
use crate::font::{FontResource, apply_font, refresh_font_cache, remove_font, state_font};
use crate::output::{apply_with_status, bold, display_target, green, red, summarize_plan, yellow};
use crate::package::{
    PackageProvider, PackageResource, package_provider_available, package_provides,
    run_provider_command,
};
use crate::plan::{
    PlanStep, font_id_for, group_id_for, package_id_for, service_id_for, state_group,
    state_package, state_service, state_user_group, user_group_id_for,
};
use crate::service::{ServiceProvider, ServiceResource, service_apply, service_remove};
use crate::state::{State, StateResource};
use crate::symlink::{apply_symlink, remove_symlink, state_symlink, symlink_id_for};
use crate::user::{
    SystemGroupResource, UserGroupResource, UserShellResource, apply_group, apply_shell,
    create_group, remove_group, remove_user_from_group,
};

pub(crate) fn apply_plan(plan: &[PlanStep], state: &mut State) -> Result<()> {
    let summary = summarize_plan(plan);
    if summary.conflicts > 0 {
        bail!(
            "plan has {} conflict(s); refusing to apply",
            summary.conflicts
        )
    }

    let tracked = track_noop_resources(plan, state);

    if summary.create + summary.update + summary.remove == 0 {
        if tracked > 0 {
            println!();
            println!("{} {} resources tracked.", bold("State updated:"), tracked);
        }
        return Ok(());
    }

    prepare_sudo(plan)?;

    println!();
    println!("{}", bold("Applying:"));

    for index in apply_order(plan)? {
        match &plan[index] {
            PlanStep::SymlinkCreate(resource) => apply_with_status(
                "Creating",
                "Create",
                &format!("symlink.{}", display_target(&resource.target)),
                || apply_symlink(resource, state),
            )?,
            PlanStep::SymlinkUpdate(resource) => apply_with_status(
                "Updating",
                "Update",
                &format!("symlink.{}", display_target(&resource.target)),
                || apply_symlink(resource, state),
            )?,
            PlanStep::SymlinkRemove { target, source, .. } => {
                let resource = StateResource::Symlink {
                    target: target.clone(),
                    source: source.clone(),
                };
                apply_with_status(
                    "Destroying",
                    "Destroy",
                    &format!("symlink.{}", display_target(target)),
                    || remove_symlink(&resource, state),
                )?
            }
            PlanStep::PackageCreate { resource, provider } => apply_with_status(
                "Installing",
                "Install",
                &format!("package.{}.{}", resource.provider, resource.name),
                || install_package(provider, resource, state),
            )?,
            PlanStep::PackageRemove { resource, provider } => apply_with_status(
                "Removing",
                "Remove",
                &format!("package.{}.{}", resource.provider, resource.name),
                || remove_package(provider, resource, state),
            )?,
            PlanStep::SymlinkNoop(resource) => {
                state
                    .resources
                    .insert(symlink_id_for(resource), state_symlink(resource));
            }
            PlanStep::PackageNoop { resource, .. } => {
                state
                    .resources
                    .insert(package_id_for(resource), state_package(resource));
            }
            PlanStep::ServiceCreate { resource, provider } => apply_with_status(
                "Applying",
                "Apply",
                &format!(
                    "service.{}.{}.{}",
                    resource.provider,
                    resource.action.as_str(),
                    resource.name
                ),
                || apply_service(provider, resource, state),
            )?,
            PlanStep::ServiceRemove { resource, provider } => apply_with_status(
                "Removing",
                "Remove",
                &format!(
                    "service.{}.{}.{}",
                    resource.provider,
                    resource.action.as_str(),
                    resource.name
                ),
                || remove_service(provider, resource, state),
            )?,
            PlanStep::ServiceNoop(resource) => {
                state
                    .resources
                    .insert(service_id_for(resource), state_service(resource));
            }
            PlanStep::FontCreate(resource) | PlanStep::FontUpdate(resource) => apply_with_status(
                "Installing",
                "Install",
                &format!("font.{}", display_target(&resource.target)),
                || install_font(resource, state),
            )?,
            PlanStep::FontRemove { source, target } => {
                let resource = crate::state::StateResource::Font {
                    source: source.clone(),
                    target: target.clone(),
                };
                apply_with_status(
                    "Removing",
                    "Remove",
                    &format!("font.{}", display_target(target)),
                    || uninstall_font(&resource, state),
                )?
            }
            PlanStep::FontNoop(resource) => {
                state
                    .resources
                    .insert(font_id_for(resource), state_font(resource));
            }
            PlanStep::CommandCreate(resource) => apply_with_status(
                "Running",
                "Run",
                &format!("command.{}", resource.name),
                || run_command(resource),
            )?,
            PlanStep::CommandNoop(_) => {}
            PlanStep::UserShellUpdate { resource, .. } => apply_with_status(
                "Updating",
                "Update",
                &format!("user.shell.{}", resource.name),
                || update_shell(resource),
            )?,
            PlanStep::SystemGroupCreate(resource) => apply_with_status(
                "Creating",
                "Create",
                &format!("group.{}", resource.name),
                || add_system_group(resource, state),
            )?,
            PlanStep::SystemGroupRemove(resource) => apply_with_status(
                "Removing",
                "Remove",
                &format!("group.{}", resource.name),
                || delete_system_group(resource, state),
            )?,
            PlanStep::UserGroupAdd(resource) => apply_with_status(
                "Adding",
                "Add",
                &format!("user.group.{}", resource.name),
                || add_group(resource, state),
            )?,
            PlanStep::UserGroupRemove(resource) => apply_with_status(
                "Removing",
                "Remove",
                &format!("user.group.{}", resource.name),
                || delete_user_group(resource, state),
            )?,
            PlanStep::UserShellNoop | PlanStep::SystemGroupNoop(_) | PlanStep::UserGroupNoop(_) => {
            }
            PlanStep::SymlinkConflict { .. }
            | PlanStep::PackageConflict { .. }
            | PlanStep::ServiceConflict { .. }
            | PlanStep::FontConflict { .. }
            | PlanStep::SystemGroupConflict { .. }
            | PlanStep::UserGroupConflict { .. }
            | PlanStep::CapabilityConflict { .. } => unreachable!(),
        }
    }

    println!();
    println!(
        "{} {} created, {} updated, {} destroyed.",
        bold("Apply complete:"),
        green(&summary.create.to_string()),
        yellow(&summary.update.to_string()),
        red(&summary.remove.to_string()),
    );

    if plan
        .iter()
        .any(|step| matches!(step, PlanStep::UserShellUpdate { .. }))
    {
        println!("Restart your login session for the shell change to take effect.");
    }

    Ok(())
}

fn prepare_sudo(plan: &[PlanStep]) -> Result<()> {
    if !plan_uses_sudo(plan) {
        return Ok(());
    }

    println!();
    println!("{}", bold("Authenticating sudo:"));
    let status = ProcessCommand::new("sudo")
        .arg("-v")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    if !status.success() {
        bail!("sudo authentication failed");
    }
    Ok(())
}

fn plan_uses_sudo(plan: &[PlanStep]) -> bool {
    plan.iter().any(|step| match step {
        PlanStep::PackageCreate { provider, .. } => shell_uses_sudo(&provider.install),
        PlanStep::PackageRemove { provider, .. } => shell_uses_sudo(&provider.remove),
        PlanStep::ServiceCreate { provider, resource } => match resource.action {
            crate::service::ServiceAction::Start => {
                provider.start.as_deref().is_some_and(shell_uses_sudo)
            }
            crate::service::ServiceAction::Enable => {
                provider.enable.as_deref().is_some_and(shell_uses_sudo)
            }
        },
        PlanStep::ServiceRemove { provider, resource } => match resource.action {
            crate::service::ServiceAction::Start => {
                provider.stop.as_deref().is_some_and(shell_uses_sudo)
            }
            crate::service::ServiceAction::Enable => {
                provider.disable.as_deref().is_some_and(shell_uses_sudo)
            }
        },
        PlanStep::CommandCreate(resource) => shell_uses_sudo(&resource.apply),
        PlanStep::SystemGroupCreate(_)
        | PlanStep::SystemGroupRemove(_)
        | PlanStep::UserGroupAdd(_)
        | PlanStep::UserGroupRemove(_) => true,
        _ => false,
    })
}

fn shell_uses_sudo(command: &str) -> bool {
    command
        .split(|character: char| {
            character.is_whitespace() || matches!(character, ';' | '&' | '|' | '(' | ')')
        })
        .any(|word| word == "sudo")
}

fn apply_order(plan: &[PlanStep]) -> Result<Vec<usize>> {
    let action_indices = plan
        .iter()
        .enumerate()
        .filter_map(|(index, step)| is_apply_step(step).then_some(index))
        .collect::<Vec<_>>();
    let action_set = action_indices.iter().copied().collect::<BTreeSet<_>>();

    let mut provider = BTreeMap::<String, usize>::new();
    let mut satisfied = BTreeSet::<String>::new();
    for (index, step) in plan.iter().enumerate() {
        let is_action = action_set.contains(&index);
        if let Some(id) = step_id(step) {
            if is_action {
                provider.entry(id).or_insert(index);
            } else {
                satisfied.insert(id);
            }
        }

        let provides = step_provides(step);
        if is_action {
            for capability in provides {
                provider.insert(capability, index);
            }
        } else {
            satisfied.extend(provides);
        }
    }
    let mut edges = BTreeMap::<usize, BTreeSet<usize>>::new();
    let mut indegree = BTreeMap::<usize, usize>::new();
    for &index in &action_indices {
        indegree.insert(index, 0);
    }

    for &index in &action_indices {
        for dependency in step_needs(&plan[index]) {
            if satisfied.contains(&dependency) {
                continue;
            }
            let Some(&dependency_index) = provider.get(&dependency) else {
                continue;
            };
            if dependency_index == index || !action_set.contains(&dependency_index) {
                continue;
            }
            if edges.entry(dependency_index).or_default().insert(index) {
                *indegree.entry(index).or_default() += 1;
            }
        }
    }

    let original_position = action_indices
        .iter()
        .enumerate()
        .map(|(position, index)| (*index, position))
        .collect::<BTreeMap<_, _>>();
    let mut ready = action_indices
        .iter()
        .copied()
        .filter(|index| indegree.get(index).copied().unwrap_or_default() == 0)
        .collect::<Vec<_>>();
    ready.sort_by_key(|index| original_position[index]);
    let mut ready = VecDeque::from(ready);
    let mut ordered = Vec::new();

    while let Some(index) = ready.pop_front() {
        ordered.push(index);
        if let Some(children) = edges.get(&index) {
            for &child in children {
                let count = indegree
                    .get_mut(&child)
                    .expect("child should have indegree");
                *count -= 1;
                if *count == 0 {
                    let position = ready
                        .iter()
                        .position(|queued| original_position[&child] < original_position[queued])
                        .unwrap_or(ready.len());
                    ready.insert(position, child);
                }
            }
        }
    }

    if ordered.len() != action_indices.len() {
        bail!("resource dependency cycle detected");
    }

    Ok(ordered)
}

fn is_apply_step(step: &PlanStep) -> bool {
    matches!(
        step,
        PlanStep::SymlinkCreate(_)
            | PlanStep::SymlinkUpdate(_)
            | PlanStep::SymlinkRemove { .. }
            | PlanStep::PackageCreate { .. }
            | PlanStep::PackageRemove { .. }
            | PlanStep::ServiceCreate { .. }
            | PlanStep::ServiceRemove { .. }
            | PlanStep::FontCreate(_)
            | PlanStep::FontUpdate(_)
            | PlanStep::FontRemove { .. }
            | PlanStep::UserShellUpdate { .. }
            | PlanStep::SystemGroupCreate(_)
            | PlanStep::SystemGroupRemove(_)
            | PlanStep::UserGroupAdd(_)
            | PlanStep::UserGroupRemove(_)
            | PlanStep::CommandCreate(_)
    )
}

fn step_id(step: &PlanStep) -> Option<String> {
    match step {
        PlanStep::CommandCreate(resource) | PlanStep::CommandNoop(resource) => {
            Some(command_id_for(resource))
        }
        PlanStep::PackageCreate { resource, .. }
        | PlanStep::PackageRemove { resource, .. }
        | PlanStep::PackageNoop { resource, .. } => Some(package_id_for(resource)),
        PlanStep::ServiceCreate { resource, .. } | PlanStep::ServiceRemove { resource, .. } => {
            Some(service_id_for(resource))
        }
        PlanStep::FontCreate(resource) | PlanStep::FontUpdate(resource) => {
            Some(font_id_for(resource))
        }
        PlanStep::FontRemove { target, .. } => Some(format!("font:{}", target.display())),
        PlanStep::SymlinkCreate(resource) | PlanStep::SymlinkUpdate(resource) => {
            Some(symlink_id_for(resource))
        }
        PlanStep::SymlinkRemove { target, .. } => Some(format!("symlink:{}", target.display())),
        PlanStep::UserShellUpdate { resource, .. } => Some(format!("user-shell:{}", resource.name)),
        PlanStep::SystemGroupCreate(resource)
        | PlanStep::SystemGroupRemove(resource)
        | PlanStep::SystemGroupNoop(resource) => Some(group_id_for(resource)),
        PlanStep::UserGroupAdd(resource)
        | PlanStep::UserGroupRemove(resource)
        | PlanStep::UserGroupNoop(resource) => Some(user_group_id_for(resource)),
        _ => None,
    }
}

fn step_provides(step: &PlanStep) -> Vec<String> {
    match step {
        PlanStep::CommandCreate(resource) | PlanStep::CommandNoop(resource) => {
            resource.provides.clone()
        }
        PlanStep::PackageCreate { resource, provider }
        | PlanStep::PackageNoop { resource, provider } => package_provides(provider, resource),
        _ => Vec::new(),
    }
}

fn step_needs(step: &PlanStep) -> Vec<String> {
    match step {
        PlanStep::CommandCreate(resource) => resource.needs.clone(),
        PlanStep::UserGroupAdd(resource) => vec![format!("group:{}", resource.name)],
        PlanStep::SystemGroupRemove(resource) => vec![format!("user-group:{}", resource.name)],
        PlanStep::PackageCreate { provider, .. } | PlanStep::PackageRemove { provider, .. } => {
            vec![provider.capability.clone()]
        }
        PlanStep::ServiceCreate { provider, .. } | PlanStep::ServiceRemove { provider, .. } => {
            vec![provider.capability.clone()]
        }
        _ => Vec::new(),
    }
}

fn track_noop_resources(plan: &[PlanStep], state: &mut State) -> usize {
    let mut tracked = 0;
    for step in plan {
        let inserted = match step {
            PlanStep::SymlinkNoop(resource) => state
                .resources
                .insert(symlink_id_for(resource), state_symlink(resource))
                .is_none(),
            PlanStep::PackageNoop { resource, .. } => state
                .resources
                .insert(package_id_for(resource), state_package(resource))
                .is_none(),
            PlanStep::ServiceNoop(resource) => state
                .resources
                .insert(service_id_for(resource), state_service(resource))
                .is_none(),
            PlanStep::FontNoop(resource) => state
                .resources
                .insert(font_id_for(resource), state_font(resource))
                .is_none(),
            PlanStep::SystemGroupNoop(resource) => state
                .resources
                .insert(group_id_for(resource), state_group(resource))
                .is_none(),
            PlanStep::UserGroupNoop(resource) => state
                .resources
                .insert(user_group_id_for(resource), state_user_group(resource))
                .is_none(),
            _ => false,
        };
        if inserted {
            tracked += 1;
        }
    }
    tracked
}

fn run_command(resource: &CommandResource) -> Result<()> {
    command_apply(resource)
}

fn update_shell(resource: &UserShellResource) -> Result<()> {
    apply_shell(resource)
}

fn add_system_group(resource: &SystemGroupResource, state: &mut State) -> Result<()> {
    create_group(resource)?;
    state
        .resources
        .insert(group_id_for(resource), state_group(resource));
    Ok(())
}

fn delete_system_group(resource: &SystemGroupResource, state: &mut State) -> Result<()> {
    remove_group(resource)?;
    state.resources.remove(&group_id_for(resource));
    Ok(())
}

fn add_group(resource: &UserGroupResource, state: &mut State) -> Result<()> {
    apply_group(resource)?;
    state
        .resources
        .insert(user_group_id_for(resource), state_user_group(resource));
    Ok(())
}

fn delete_user_group(resource: &UserGroupResource, state: &mut State) -> Result<()> {
    remove_user_from_group(resource)?;
    state.resources.remove(&user_group_id_for(resource));
    Ok(())
}

fn install_font(resource: &FontResource, state: &mut State) -> Result<()> {
    apply_font(resource, state)?;
    refresh_font_cache()?;
    Ok(())
}

fn uninstall_font(resource: &StateResource, state: &mut State) -> Result<()> {
    remove_font(resource, state)?;
    refresh_font_cache()?;
    Ok(())
}

fn apply_service(
    provider: &ServiceProvider,
    resource: &ServiceResource,
    state: &mut State,
) -> Result<()> {
    service_apply(provider, resource)?;
    state
        .resources
        .insert(service_id_for(resource), state_service(resource));
    Ok(())
}

fn remove_service(
    provider: &ServiceProvider,
    resource: &ServiceResource,
    state: &mut State,
) -> Result<()> {
    service_remove(provider, resource)?;
    state.resources.remove(&service_id_for(resource));
    Ok(())
}

fn install_package(
    provider: &PackageProvider,
    resource: &PackageResource,
    state: &mut State,
) -> Result<()> {
    if !package_provider_available(provider)? {
        bail!("{} is not available", resource.provider);
    }
    if !run_provider_command(&provider.install, Some(&resource.name), false)? {
        bail!("{} failed to install {}", resource.provider, resource.name);
    }
    state
        .resources
        .insert(package_id_for(resource), state_package(resource));
    Ok(())
}

fn remove_package(
    provider: &PackageProvider,
    resource: &PackageResource,
    state: &mut State,
) -> Result<()> {
    if !package_provider_available(provider)? {
        bail!("{} is not available", resource.provider);
    }
    if !run_provider_command(&provider.remove, Some(&resource.name), false)? {
        bail!("{} failed to remove {}", resource.provider, resource.name);
    }
    state.resources.remove(&package_id_for(resource));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(capability: &str) -> PackageProvider {
        PackageProvider {
            capability: capability.to_string(),
            available: "exit 0".to_string(),
            installed: "exit 1".to_string(),
            install: "exit 0".to_string(),
            remove: "exit 0".to_string(),
            list: None,
            package_provides: BTreeMap::new(),
            matcher: crate::package::PackageMatcher::Exact,
        }
    }

    #[test]
    fn apply_order_uses_command_provided_capabilities() {
        let package = PlanStep::PackageCreate {
            resource: PackageResource {
                provider: "brew".to_string(),
                name: "bat".to_string(),
            },
            provider: provider("provider:brew"),
        };
        let command = PlanStep::CommandCreate(CommandResource {
            name: "homebrew".to_string(),
            check: "exit 1".to_string(),
            apply: "exit 0".to_string(),
            needs: Vec::new(),
            provides: vec!["provider:brew".to_string()],
        });
        let plan = vec![package, command];

        let order = apply_order(&plan).unwrap();

        assert_eq!(order, vec![1, 0]);
    }

    #[test]
    fn apply_order_uses_package_provided_capabilities() {
        let paru_package = PlanStep::PackageCreate {
            resource: PackageResource {
                provider: "paru".to_string(),
                name: "bat".to_string(),
            },
            provider: provider("provider:paru"),
        };
        let mut pacman_provider = provider("provider:pacman");
        pacman_provider
            .package_provides
            .insert("paru".to_string(), "provider:paru".to_string());
        let pacman_paru = PlanStep::PackageCreate {
            resource: PackageResource {
                provider: "pacman".to_string(),
                name: "paru".to_string(),
            },
            provider: pacman_provider,
        };
        let plan = vec![paru_package, pacman_paru];

        let order = apply_order(&plan).unwrap();

        assert_eq!(order, vec![1, 0]);
    }

    #[test]
    fn apply_order_uses_explicit_command_needs() {
        let pi = PlanStep::CommandCreate(CommandResource {
            name: "pi".to_string(),
            check: "exit 1".to_string(),
            apply: "exit 0".to_string(),
            needs: vec!["command:mise tools".to_string()],
            provides: Vec::new(),
        });
        let mise = PlanStep::CommandCreate(CommandResource {
            name: "mise tools".to_string(),
            check: "exit 1".to_string(),
            apply: "exit 0".to_string(),
            needs: Vec::new(),
            provides: Vec::new(),
        });
        let plan = vec![pi, mise];

        let order = apply_order(&plan).unwrap();

        assert_eq!(order, vec![1, 0]);
    }
}
