use anyhow::{Result, bail};

use crate::output::{apply_with_status, bold, display_target, green, red, summarize_plan, yellow};
use crate::package::{
    PackageProvider, PackageResource, package_provider_available, run_provider_command,
};
use crate::plan::{PlanStep, package_id_for, service_id_for, state_package, state_service};
use crate::service::{ServiceProvider, ServiceResource, service_apply, service_remove};
use crate::state::{State, StateResource};
use crate::symlink::{apply_symlink, remove_symlink, state_symlink, symlink_id_for};

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

    println!();
    println!("{}", bold("Applying:"));

    for step in plan {
        match step {
            PlanStep::SymlinkCreate(resource) => apply_with_status(
                "Creating",
                "Creation",
                &format!("symlink.{}", display_target(&resource.target)),
                || apply_symlink(resource, state),
            )?,
            PlanStep::SymlinkUpdate(resource) => apply_with_status(
                "Updating",
                "Update",
                &format!("symlink.{}", display_target(&resource.target)),
                || apply_symlink(resource, state),
            )?,
            PlanStep::SymlinkRemove { target, source } => {
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
            PlanStep::PackageNoop(resource) => {
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
            PlanStep::SymlinkConflict { .. }
            | PlanStep::PackageConflict { .. }
            | PlanStep::ServiceConflict { .. } => unreachable!(),
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

    Ok(())
}

fn track_noop_resources(plan: &[PlanStep], state: &mut State) -> usize {
    let mut tracked = 0;
    for step in plan {
        let inserted = match step {
            PlanStep::SymlinkNoop(resource) => state
                .resources
                .insert(symlink_id_for(resource), state_symlink(resource))
                .is_none(),
            PlanStep::PackageNoop(resource) => state
                .resources
                .insert(package_id_for(resource), state_package(resource))
                .is_none(),
            PlanStep::ServiceNoop(resource) => state
                .resources
                .insert(service_id_for(resource), state_service(resource))
                .is_none(),
            _ => false,
        };
        if inserted {
            tracked += 1;
        }
    }
    tracked
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
