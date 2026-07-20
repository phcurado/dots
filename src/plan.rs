use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use anyhow::Result;

use crate::command::{CommandResource, command_current};
use crate::config::Config;
use crate::docker::{
    ComposeResource, compose_available, compose_current, compose_from_state, compose_id_for,
    state_compose,
};
use crate::font::{FontResource, font_matches, state_font};
use crate::managed_file::{FileResource, FileStatus, file_id_for, inspect_file, state_file};
use crate::managed_output::{OutputValue, resolve_outputs};
use crate::package::{
    PackageProvider, PackageResource, PackageStatusCache, package_installed_cached,
    package_provider_available, package_provides,
};
use crate::service::{
    ServiceProvider, ServiceResource, ServiceStatusCache, service_current_cached,
    service_provider_available,
};
use crate::ssh::{
    KeypairObservation, KeypairStatus, SshKeypairResource, inspect_keypair, keypair_id_for,
    state_keypair,
};
use crate::state::{State, StateResource};
use crate::symlink::{
    SymlinkCandidate, SymlinkResource, regular_file_matches, resolve_symlink_target, same_path,
    stale_symlinks_for_declaration, state_symlink, symlink_candidate_for_resource, symlink_id_for,
    symlink_matches,
};
use crate::systemd::{
    SystemdUnitResource, systemd_available, systemd_unit_from_state, systemd_unit_id_for,
    unit_current, unit_file_matches, unit_installed,
};
use crate::user::{
    SystemGroupResource, UserGroupResource, UserShellResource, current_shell, shell_matches,
    system_group_exists, user_in_group,
};

#[derive(Debug, Clone)]
pub(crate) enum SymlinkConflictReason {
    MissingSource { current_target: Option<PathBuf> },
    TargetUnmanaged,
    TargetExistsDifferentContent,
    TargetExistsNotSymlink,
}

#[derive(Debug, Clone)]
pub(crate) enum PlanStep {
    SymlinkCreate(SymlinkResource),
    SymlinkUpdate(SymlinkResource),
    SymlinkRemove {
        target: PathBuf,
        source: PathBuf,
        stale: bool,
    },
    SymlinkNoop(SymlinkResource),
    SymlinkConflict {
        resource: SymlinkResource,
        reason: SymlinkConflictReason,
    },
    SymlinkCandidate(SymlinkCandidate),
    PackageCreate {
        resource: PackageResource,
        provider: PackageProvider,
    },
    PackageRemove {
        resource: PackageResource,
        provider: PackageProvider,
    },
    PackageNoop {
        resource: PackageResource,
        provider: PackageProvider,
    },
    PackageConflict {
        resource: PackageResource,
        reason: String,
    },
    ServiceCreate {
        resource: ServiceResource,
        provider: ServiceProvider,
    },
    ServiceRemove {
        resource: ServiceResource,
        provider: ServiceProvider,
    },
    ServiceNoop(ServiceResource),
    ServiceConflict {
        resource: ServiceResource,
        reason: String,
    },
    SystemdUnitCreate(SystemdUnitResource),
    SystemdUnitUpdate(SystemdUnitResource),
    SystemdUnitRemove(SystemdUnitResource),
    SystemdUnitNoop(SystemdUnitResource),
    SystemdUnitConflict {
        resource: SystemdUnitResource,
        reason: String,
    },
    ComposeCreate(ComposeResource),
    ComposeUpdate(ComposeResource),
    ComposeRemove {
        resource: ComposeResource,
        stored_config: String,
    },
    ComposeNoop {
        resource: ComposeResource,
        fingerprint: String,
    },
    ComposeConflict {
        resource: ComposeResource,
        reason: String,
    },
    FontCreate(FontResource),
    FontUpdate(FontResource),
    FontRemove {
        source: PathBuf,
        target: PathBuf,
    },
    FontNoop(FontResource),
    FontConflict {
        resource: FontResource,
        reason: String,
    },
    FileCreate(FileResource),
    FileUpdate(FileResource),
    FileModeUpdate(FileResource),
    FileNoop(FileResource),
    FileForget {
        target: PathBuf,
    },
    FileConflict {
        resource: FileResource,
        reason: String,
    },
    SshKeypairCreate(SshKeypairResource),
    SshKeypairAdopt(SshKeypairResource),
    SshKeypairPermissionUpdate {
        resource: SshKeypairResource,
        observation: KeypairObservation,
    },
    SshKeypairNoop {
        resource: SshKeypairResource,
        observation: KeypairObservation,
    },
    SshKeypairForget {
        name: String,
    },
    SshKeypairConflict {
        resource: SshKeypairResource,
        reason: String,
    },
    OutputCreate {
        name: String,
        value: Option<serde_json::Value>,
    },
    OutputUpdate {
        name: String,
        before: serde_json::Value,
        after: Option<serde_json::Value>,
    },
    OutputRemove {
        name: String,
        value: serde_json::Value,
    },
    UserShellUpdate {
        resource: UserShellResource,
        current: Option<PathBuf>,
    },
    UserShellNoop,
    SystemGroupCreate(SystemGroupResource),
    SystemGroupRemove(SystemGroupResource),
    SystemGroupNoop(SystemGroupResource),
    SystemGroupConflict {
        resource: SystemGroupResource,
        reason: String,
    },
    UserGroupAdd(UserGroupResource),
    UserGroupRemove(UserGroupResource),
    UserGroupNoop(UserGroupResource),
    UserGroupConflict {
        resource: UserGroupResource,
        reason: String,
    },
    CommandCreate(CommandResource),
    CommandNoop(CommandResource),
    CapabilityConflict {
        capability: String,
        reason: String,
    },
}

fn missing_symlink_source_reason(resource: &SymlinkResource) -> Result<SymlinkConflictReason> {
    let current_target = if fs::symlink_metadata(&resource.target)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        let current = fs::read_link(&resource.target)?;
        Some(resolve_symlink_target(&resource.target, &current))
    } else {
        None
    };
    Ok(SymlinkConflictReason::MissingSource { current_target })
}

pub(crate) fn refresh_state_from_system(config: &Config, state: &mut State) -> Result<()> {
    for resource in &config.symlinks {
        if symlink_matches(resource)? {
            state
                .resources
                .insert(symlink_id_for(resource), state_symlink(resource));
        }
    }

    let mut package_status = PackageStatusCache::default();
    for resource in &config.packages {
        let Some(provider) = config.package_providers.get(&resource.provider) else {
            continue;
        };
        if package_provider_available(provider)?
            && package_installed_cached(&mut package_status, provider, resource)?
        {
            state
                .resources
                .insert(package_id_for(resource), state_package(resource));
        }
    }

    let mut service_status = ServiceStatusCache::default();
    for resource in &config.services {
        let Some(provider) = config.service_providers.get(&resource.provider) else {
            continue;
        };
        if service_provider_available(provider)?
            && service_current_cached(&mut service_status, provider, resource)?
        {
            state
                .resources
                .insert(service_id_for(resource), state_service(resource));
        }
    }

    if systemd_available() {
        for resource in &config.systemd_units {
            if resource.file.exists() && unit_current(resource)? {
                state.resources.insert(
                    systemd_unit_id_for(resource),
                    crate::systemd::state_systemd_unit(resource),
                );
            }
        }
    }

    if compose_available()? {
        for resource in &config.compose {
            let id = compose_id_for(resource);
            let fingerprint = state
                .resources
                .get(&id)
                .and_then(compose_from_state)
                .map(|(_, fingerprint)| fingerprint);
            if compose_current(resource, fingerprint)? {
                let fingerprint = fingerprint
                    .expect("current compose resource has a fingerprint")
                    .to_string();
                state
                    .resources
                    .insert(id, state_compose(resource, fingerprint));
            }
        }
    }

    for resource in &config.fonts {
        if font_matches(resource)? {
            state
                .resources
                .insert(font_id_for(resource), state_font(resource));
        }
    }

    for resource in &config.files {
        if inspect_file(resource)? == FileStatus::Current {
            state
                .resources
                .insert(file_id_for(resource), state_file(resource)?);
        }
    }

    for resource in &config.ssh_keypairs {
        match inspect_keypair(resource, state)? {
            KeypairStatus::Current(observation) | KeypairStatus::PermissionDrift(observation) => {
                state.resources.insert(
                    keypair_id_for(resource),
                    state_keypair(resource, &observation),
                );
            }
            _ => {}
        }
    }

    if std::env::consts::OS == "linux" {
        for resource in &config.user.groups {
            if system_group_exists(&resource.name)? {
                state
                    .resources
                    .insert(group_id_for(resource), state_group(resource));
            }
        }

        for resource in &config.user.memberships {
            if user_in_group(resource)? {
                state
                    .resources
                    .insert(user_group_id_for(resource), state_user_group(resource));
            }
        }
    }

    Ok(())
}

pub(crate) fn build_plan(config: &Config, state: &State) -> Result<Vec<PlanStep>> {
    let mut plan = Vec::new();
    let mut declared = BTreeSet::new();
    let mut declared_symlink_targets = BTreeSet::new();
    let mut planned_symlink_removals = BTreeSet::new();
    let mut symlink_candidate_targets = BTreeSet::new();

    for resource in &config.symlinks {
        let id = symlink_id_for(resource);
        declared.insert(id.clone());
        declared_symlink_targets.insert(resource.target.clone());
        let owned = state.resources.contains_key(&id);

        if !resource.source.exists() {
            if let Some(candidate) = symlink_candidate_for_resource(resource)? {
                symlink_candidate_targets.insert(candidate.target.clone());
                plan.push(PlanStep::SymlinkCandidate(candidate));
            } else {
                plan.push(PlanStep::SymlinkConflict {
                    resource: resource.clone(),
                    reason: missing_symlink_source_reason(resource)?,
                });
            }
            continue;
        }

        match fs::symlink_metadata(&resource.target) {
            Ok(meta) if meta.file_type().is_symlink() => {
                let current = fs::read_link(&resource.target)?;
                let current = resolve_symlink_target(&resource.target, &current);
                if owned && same_path(&current, &resource.source) {
                    plan.push(PlanStep::SymlinkNoop(resource.clone()));
                } else if owned {
                    plan.push(PlanStep::SymlinkUpdate(resource.clone()));
                } else if same_path(&current, &resource.source) {
                    plan.push(PlanStep::SymlinkNoop(resource.clone()));
                } else {
                    plan.push(PlanStep::SymlinkConflict {
                        resource: resource.clone(),
                        reason: SymlinkConflictReason::TargetUnmanaged,
                    });
                }
            }
            Ok(meta) if meta.is_file() && regular_file_matches(resource)? => {
                plan.push(PlanStep::SymlinkUpdate(resource.clone()));
            }
            Ok(meta) if meta.is_file() => plan.push(PlanStep::SymlinkConflict {
                resource: resource.clone(),
                reason: SymlinkConflictReason::TargetExistsDifferentContent,
            }),
            Ok(_) => plan.push(PlanStep::SymlinkConflict {
                resource: resource.clone(),
                reason: SymlinkConflictReason::TargetExistsNotSymlink,
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                plan.push(PlanStep::SymlinkCreate(resource.clone()));
            }
            Err(error) => return Err(error.into()),
        }
    }

    for declaration in &config.symlink_declarations {
        for resource in stale_symlinks_for_declaration(declaration, &declared_symlink_targets)? {
            if planned_symlink_removals.insert(resource.target.clone()) {
                plan.push(PlanStep::SymlinkRemove {
                    target: resource.target,
                    source: resource.source,
                    stale: true,
                });
            }
        }
    }

    let (command_steps, mut provided_capabilities) = plan_commands(&config.commands)?;
    for resource in &config.packages {
        if let Some(provider) = config.package_providers.get(&resource.provider) {
            provided_capabilities.extend(package_provides(provider, resource));
        }
    }

    let mut missing_capabilities = BTreeSet::new();
    let mut package_status = PackageStatusCache::default();
    for resource in &config.packages {
        let id = package_id_for(resource);
        declared.insert(id.clone());

        let Some(provider) = config.package_providers.get(&resource.provider) else {
            plan.push(PlanStep::PackageConflict {
                resource: resource.clone(),
                reason: format!("{} provider is not configured", resource.provider),
            });
            continue;
        };
        let capability = &provider.capability;
        if !provided_capabilities.contains(capability) && !package_provider_available(provider)? {
            if missing_capabilities.insert(capability.clone()) {
                plan.push(PlanStep::CapabilityConflict {
                    capability: provider.capability_name(),
                    reason: "is not available".to_string(),
                });
            }
            continue;
        }
        if package_installed_cached(&mut package_status, provider, resource)? {
            plan.push(PlanStep::PackageNoop {
                resource: resource.clone(),
                provider: provider.clone(),
            });
        } else {
            plan.push(PlanStep::PackageCreate {
                resource: resource.clone(),
                provider: provider.clone(),
            });
        }
    }

    let systemd_is_available = systemd_available();
    for resource in &config.systemd_units {
        let id = systemd_unit_id_for(resource);
        declared.insert(id.clone());
        if !resource.file.exists() {
            plan.push(PlanStep::SystemdUnitConflict {
                resource: resource.clone(),
                reason: format!("source does not exist: {}", resource.file.display()),
            });
        } else if !systemd_is_available {
            plan.push(PlanStep::SystemdUnitConflict {
                resource: resource.clone(),
                reason: "systemd is not available".to_string(),
            });
        } else if unit_current(resource)? {
            plan.push(PlanStep::SystemdUnitNoop(resource.clone()));
        } else if state.resources.contains_key(&id) {
            plan.push(PlanStep::SystemdUnitUpdate(resource.clone()));
        } else if unit_file_matches(resource)? || !unit_installed(resource) {
            plan.push(PlanStep::SystemdUnitCreate(resource.clone()));
        } else {
            plan.push(PlanStep::SystemdUnitConflict {
                resource: resource.clone(),
                reason: "installed service is not managed".to_string(),
            });
        }
    }

    let compose_is_available = compose_available()?;
    for resource in &config.compose {
        let id = compose_id_for(resource);
        declared.insert(id.clone());
        let stored = state.resources.get(&id).and_then(compose_from_state);
        if !compose_is_available {
            plan.push(PlanStep::ComposeConflict {
                resource: resource.clone(),
                reason: "docker compose is not available".to_string(),
            });
        } else if compose_current(
            resource,
            stored.as_ref().map(|(_, fingerprint)| *fingerprint),
        )? {
            plan.push(PlanStep::ComposeNoop {
                resource: resource.clone(),
                fingerprint: stored
                    .expect("current compose resource has state")
                    .1
                    .to_string(),
            });
        } else if state.resources.contains_key(&id) {
            plan.push(PlanStep::ComposeUpdate(resource.clone()));
        } else {
            plan.push(PlanStep::ComposeCreate(resource.clone()));
        }
    }

    for resource in &config.ssh_keypairs {
        let id = keypair_id_for(resource);
        declared.insert(id);
        match inspect_keypair(resource, state)? {
            KeypairStatus::Missing => plan.push(PlanStep::SshKeypairCreate(resource.clone())),
            KeypairStatus::Current(observation) => plan.push(PlanStep::SshKeypairNoop {
                resource: resource.clone(),
                observation,
            }),
            KeypairStatus::PermissionDrift(observation) => {
                plan.push(PlanStep::SshKeypairPermissionUpdate {
                    resource: resource.clone(),
                    observation,
                })
            }
            KeypairStatus::ValidationRequired => {
                plan.push(PlanStep::SshKeypairAdopt(resource.clone()))
            }
            KeypairStatus::Conflict(reason) => plan.push(PlanStep::SshKeypairConflict {
                resource: resource.clone(),
                reason,
            }),
        }
    }

    for resource in &config.files {
        let id = file_id_for(resource);
        declared.insert(id.clone());
        let owned = state.resources.contains_key(&id);
        let source = match fs::symlink_metadata(&resource.source) {
            Ok(metadata) if metadata.is_file() => true,
            Ok(_) => {
                plan.push(PlanStep::FileConflict {
                    resource: resource.clone(),
                    reason: "source is not a regular file".to_string(),
                });
                false
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                plan.push(PlanStep::FileConflict {
                    resource: resource.clone(),
                    reason: format!("source does not exist: {}", resource.source.display()),
                });
                false
            }
            Err(error) => return Err(error.into()),
        };
        if !source {
            continue;
        }

        match inspect_file(resource)? {
            FileStatus::Missing => plan.push(PlanStep::FileCreate(resource.clone())),
            FileStatus::Current => plan.push(PlanStep::FileNoop(resource.clone())),
            FileStatus::ModeChanged if owned => {
                plan.push(PlanStep::FileModeUpdate(resource.clone()))
            }
            FileStatus::ContentChanged | FileStatus::ContentAndModeChanged if owned => {
                plan.push(PlanStep::FileUpdate(resource.clone()))
            }
            FileStatus::ContentChanged
            | FileStatus::ModeChanged
            | FileStatus::ContentAndModeChanged => plan.push(PlanStep::FileConflict {
                resource: resource.clone(),
                reason: "target exists but is not managed".to_string(),
            }),
            FileStatus::Conflict => plan.push(PlanStep::FileConflict {
                resource: resource.clone(),
                reason: "target is not a regular file".to_string(),
            }),
        }
    }

    for resource in &config.fonts {
        let id = font_id_for(resource);
        declared.insert(id.clone());
        let owned = state.resources.contains_key(&id);

        if !resource.source.exists() {
            plan.push(PlanStep::FontConflict {
                resource: resource.clone(),
                reason: format!("source does not exist: {}", resource.source.display()),
            });
            continue;
        }

        if font_matches(resource)? {
            plan.push(PlanStep::FontNoop(resource.clone()));
        } else if owned && resource.target.exists() {
            plan.push(PlanStep::FontUpdate(resource.clone()));
        } else {
            plan.push(PlanStep::FontCreate(resource.clone()));
        }
    }

    plan.extend(command_steps);

    let mut service_status = ServiceStatusCache::default();
    for resource in &config.services {
        let id = service_id_for(resource);
        declared.insert(id.clone());

        let Some(provider) = config.service_providers.get(&resource.provider) else {
            plan.push(PlanStep::ServiceConflict {
                resource: resource.clone(),
                reason: format!("{} service provider is not configured", resource.provider),
            });
            continue;
        };
        let capability = &provider.capability;
        if !provided_capabilities.contains(capability) && !service_provider_available(provider)? {
            if missing_capabilities.insert(capability.clone()) {
                plan.push(PlanStep::CapabilityConflict {
                    capability: provider.capability_name(),
                    reason: "is not available".to_string(),
                });
            }
            continue;
        }
        if service_current_cached(&mut service_status, provider, resource)? {
            plan.push(PlanStep::ServiceNoop(resource.clone()));
        } else {
            plan.push(PlanStep::ServiceCreate {
                resource: resource.clone(),
                provider: provider.clone(),
            });
        }
    }

    if let Some(resource) = &config.user.shell {
        if shell_matches(resource) {
            plan.push(PlanStep::UserShellNoop);
        } else {
            plan.push(PlanStep::UserShellUpdate {
                resource: resource.clone(),
                current: current_shell(),
            });
        }
    }

    let declared_groups = config
        .user
        .groups
        .iter()
        .map(|resource| resource.name.clone())
        .collect::<BTreeSet<_>>();

    for resource in &config.user.groups {
        declared.insert(group_id_for(resource));
        if std::env::consts::OS != "linux" {
            plan.push(PlanStep::SystemGroupConflict {
                resource: resource.clone(),
                reason: "groups are only supported on Linux".to_string(),
            });
            continue;
        }
        if system_group_exists(&resource.name)? {
            plan.push(PlanStep::SystemGroupNoop(resource.clone()));
        } else {
            plan.push(PlanStep::SystemGroupCreate(resource.clone()));
        }
    }

    for resource in &config.user.memberships {
        declared.insert(group_id_for(&SystemGroupResource {
            name: resource.name.clone(),
        }));
        declared.insert(user_group_id_for(resource));
        if std::env::consts::OS != "linux" {
            plan.push(PlanStep::UserGroupConflict {
                resource: resource.clone(),
                reason: "user groups are only supported on Linux".to_string(),
            });
            continue;
        }
        if !declared_groups.contains(&resource.name) && !system_group_exists(&resource.name)? {
            plan.push(PlanStep::UserGroupConflict {
                resource: resource.clone(),
                reason: "group does not exist".to_string(),
            });
        } else if user_in_group(resource)? {
            plan.push(PlanStep::UserGroupNoop(resource.clone()));
        } else {
            plan.push(PlanStep::UserGroupAdd(resource.clone()));
        }
    }

    let resolved_outputs = resolve_outputs(&config.outputs, state)?;
    let declared_outputs = config
        .outputs
        .iter()
        .map(|output| output.name.clone())
        .collect::<BTreeSet<_>>();
    for output in &config.outputs {
        let pending = match &output.value {
            OutputValue::ResourceAttribute(reference) => plan.iter().any(|step| match step {
                PlanStep::SshKeypairCreate(resource) | PlanStep::SshKeypairAdopt(resource) => {
                    keypair_id_for(resource) == reference.resource_id
                }
                _ => false,
            }),
            OutputValue::Literal(_) => false,
        };
        let resolved = (!pending)
            .then(|| resolved_outputs.get(&output.name))
            .flatten();
        match (state.outputs.get(&output.name), resolved) {
            (None, value) => plan.push(PlanStep::OutputCreate {
                name: output.name.clone(),
                value: value.cloned(),
            }),
            (Some(before), Some(after)) if before != after => plan.push(PlanStep::OutputUpdate {
                name: output.name.clone(),
                before: before.clone(),
                after: Some(after.clone()),
            }),
            (Some(before), None) => plan.push(PlanStep::OutputUpdate {
                name: output.name.clone(),
                before: before.clone(),
                after: None,
            }),
            _ => {}
        }
    }
    for (name, value) in &state.outputs {
        if !declared_outputs.contains(name) {
            plan.push(PlanStep::OutputRemove {
                name: name.clone(),
                value: value.clone(),
            });
        }
    }

    let mut deferred_brew_tap_removals = Vec::new();
    for (id, resource) in &state.resources {
        if declared.contains(id) {
            continue;
        }
        match resource {
            StateResource::Symlink { target, source } => {
                if planned_symlink_removals.insert(target.clone()) {
                    plan.push(PlanStep::SymlinkRemove {
                        target: target.clone(),
                        source: source.clone(),
                        stale: false,
                    });
                }
            }
            StateResource::Font { source, target } => plan.push(PlanStep::FontRemove {
                source: source.clone(),
                target: target.clone(),
            }),
            StateResource::File { target, .. } => plan.push(PlanStep::FileForget {
                target: target.clone(),
            }),
            StateResource::SshKeypair { name, .. } => {
                plan.push(PlanStep::SshKeypairForget { name: name.clone() })
            }
            StateResource::Package { provider, name } => {
                let resource = PackageResource {
                    provider: provider.clone(),
                    name: name.clone(),
                };
                match config.package_providers.get(provider) {
                    Some(package_provider) => {
                        let step = PlanStep::PackageRemove {
                            resource,
                            provider: package_provider.clone(),
                        };
                        if provider == "brew-tap" {
                            deferred_brew_tap_removals.push(step);
                        } else {
                            plan.push(step);
                        }
                    }
                    None => plan.push(PlanStep::PackageConflict {
                        resource,
                        reason: format!("{provider} provider is not configured"),
                    }),
                }
            }
            StateResource::Service {
                provider,
                action,
                name,
            } => {
                let resource = ServiceResource {
                    provider: provider.clone(),
                    action: *action,
                    name: name.clone(),
                };
                match config.service_providers.get(provider) {
                    Some(provider) => plan.push(PlanStep::ServiceRemove {
                        resource,
                        provider: provider.clone(),
                    }),
                    None => plan.push(PlanStep::ServiceConflict {
                        resource,
                        reason: format!("{provider} service provider is not configured"),
                    }),
                }
            }
            StateResource::SystemdUnit { .. } => {
                let resource =
                    systemd_unit_from_state(resource).expect("systemd unit state resource");
                if systemd_is_available {
                    plan.push(PlanStep::SystemdUnitRemove(resource));
                } else {
                    plan.push(PlanStep::SystemdUnitConflict {
                        resource,
                        reason: "systemd is not available".to_string(),
                    });
                }
            }
            StateResource::Compose { .. } => {
                let (resource, stored_config) =
                    compose_from_state(resource).expect("compose state resource");
                if compose_is_available {
                    plan.push(PlanStep::ComposeRemove {
                        resource,
                        stored_config: stored_config.to_string(),
                    });
                } else {
                    plan.push(PlanStep::ComposeConflict {
                        resource,
                        reason: "docker compose is not available".to_string(),
                    });
                }
            }
            StateResource::Group { name } => {
                plan.push(PlanStep::SystemGroupRemove(SystemGroupResource {
                    name: name.clone(),
                }))
            }
            StateResource::UserGroup { name } => {
                plan.push(PlanStep::UserGroupRemove(UserGroupResource {
                    name: name.clone(),
                }))
            }
        }
    }
    plan.extend(deferred_brew_tap_removals);

    Ok(plan)
}

fn plan_commands(commands: &[CommandResource]) -> Result<(Vec<PlanStep>, BTreeSet<String>)> {
    let mut plan = Vec::new();
    let mut provided = BTreeSet::new();
    for resource in commands {
        provided.extend(resource.provides.iter().cloned());
        if command_current(resource)? {
            plan.push(PlanStep::CommandNoop(resource.clone()));
        } else {
            plan.push(PlanStep::CommandCreate(resource.clone()));
        }
    }
    Ok((plan, provided))
}

pub(crate) fn state_package(resource: &PackageResource) -> StateResource {
    StateResource::Package {
        provider: resource.provider.clone(),
        name: resource.name.clone(),
    }
}

pub(crate) fn package_id_for(resource: &PackageResource) -> String {
    format!("package:{}:{}", resource.provider, resource.name)
}

pub(crate) fn state_service(resource: &ServiceResource) -> StateResource {
    StateResource::Service {
        provider: resource.provider.clone(),
        action: resource.action,
        name: resource.name.clone(),
    }
}

pub(crate) fn state_group(resource: &SystemGroupResource) -> StateResource {
    StateResource::Group {
        name: resource.name.clone(),
    }
}

pub(crate) fn group_id_for(resource: &SystemGroupResource) -> String {
    format!("group:{}", resource.name)
}

pub(crate) fn state_user_group(resource: &UserGroupResource) -> StateResource {
    StateResource::UserGroup {
        name: resource.name.clone(),
    }
}

pub(crate) fn user_group_id_for(resource: &UserGroupResource) -> String {
    format!("user-group:{}", resource.name)
}

pub(crate) fn service_id_for(resource: &ServiceResource) -> String {
    format!(
        "service:{}:{}:{}",
        resource.provider,
        resource.action.as_str(),
        resource.name
    )
}

pub(crate) fn font_id_for(resource: &FontResource) -> String {
    format!("font:{}", resource.target.display())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn fake_provider(available: &str, installed: &str) -> PackageProvider {
        PackageProvider {
            capability: "provider:fake".to_string(),
            available: available.to_string(),
            installed: installed.to_string(),
            install: "exit 0".to_string(),
            remove: "exit 0".to_string(),
            list: None,
            package_provides: BTreeMap::new(),
            matcher: crate::package::PackageMatcher::Exact,
        }
    }

    #[test]
    fn missing_provider_is_a_capability_conflict() {
        let mut config = Config::default();
        config
            .package_providers
            .insert("fake".to_string(), fake_provider("exit 1", "exit 1"));
        config.packages.push(PackageResource {
            provider: "fake".to_string(),
            name: "bat".to_string(),
        });

        let plan = build_plan(&config, &State::default()).unwrap();

        assert!(matches!(
            plan.as_slice(),
            [PlanStep::CapabilityConflict { capability, .. }] if capability == "fake"
        ));
    }

    #[test]
    fn planned_provider_skips_availability_conflict() {
        let mut config = Config::default();
        config.commands.push(CommandResource {
            name: "fake provider".to_string(),
            check: "exit 1".to_string(),
            apply: "exit 0".to_string(),
            needs: Vec::new(),
            provides: vec!["provider:fake".to_string()],
        });
        config
            .package_providers
            .insert("fake".to_string(), fake_provider("exit 1", "exit 1"));
        config.packages.push(PackageResource {
            provider: "fake".to_string(),
            name: "bat".to_string(),
        });

        let plan = build_plan(&config, &State::default()).unwrap();

        assert!(matches!(
            plan.as_slice(),
            [PlanStep::PackageCreate { .. }, PlanStep::CommandCreate(_)]
        ));
    }

    #[test]
    fn installed_packages_are_tracked_by_refresh_when_declared() {
        let mut config = Config::default();
        config
            .package_providers
            .insert("fake".to_string(), fake_provider("exit 0", "exit 0"));
        config.packages.push(PackageResource {
            provider: "fake".to_string(),
            name: "bat".to_string(),
        });
        let mut state = State::default();

        refresh_state_from_system(&config, &mut state).unwrap();
        let plan = build_plan(&config, &state).unwrap();

        assert!(state.resources.contains_key("package:fake:bat"));
        assert!(matches!(plan.as_slice(), [PlanStep::PackageNoop { .. }]));
    }

    #[test]
    fn brew_taps_are_removed_after_formulae() {
        let mut config = Config::default();
        config
            .package_providers
            .insert("brew".to_string(), fake_provider("exit 0", "exit 0"));
        config
            .package_providers
            .insert("brew-tap".to_string(), fake_provider("exit 0", "exit 0"));
        let mut state = State::default();
        state.resources.insert(
            "package:brew-tap:example/tools".to_string(),
            StateResource::Package {
                provider: "brew-tap".to_string(),
                name: "example/tools".to_string(),
            },
        );
        state.resources.insert(
            "package:brew:widget".to_string(),
            StateResource::Package {
                provider: "brew".to_string(),
                name: "widget".to_string(),
            },
        );

        let plan = build_plan(&config, &state).unwrap();

        assert!(matches!(
            plan.as_slice(),
            [
                PlanStep::PackageRemove { resource: formula, .. },
                PlanStep::PackageRemove { resource: tap, .. }
            ] if formula.provider == "brew" && tap.provider == "brew-tap"
        ));
    }

    #[test]
    fn identical_regular_symlink_target_can_be_replaced() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("target");
        let source = root.path().join("source");
        fs::write(&target, "same").unwrap();
        fs::write(&source, "same").unwrap();

        let mut config = Config::default();
        config.symlinks.push(SymlinkResource { target, source });

        let plan = build_plan(&config, &State::default()).unwrap();

        assert!(matches!(plan.as_slice(), [PlanStep::SymlinkUpdate(_)]));
    }

    #[test]
    fn untracked_stale_symlinks_under_managed_directory_are_removed() {
        let root = tempfile::tempdir().unwrap();
        let source_root = root.path().join("source");
        let target_root = root.path().join("target");
        fs::create_dir_all(&source_root).unwrap();
        fs::create_dir_all(&target_root).unwrap();
        fs::write(source_root.join("current"), "current").unwrap();
        std::os::unix::fs::symlink(source_root.join("old"), target_root.join("old")).unwrap();

        let mut config = Config::default();
        config
            .symlink_declarations
            .push(crate::symlink::SymlinkDeclaration {
                target: target_root.clone(),
                source: source_root.clone(),
                ignore: Vec::new(),
            });
        config.symlinks.push(SymlinkResource {
            target: target_root.join("current"),
            source: source_root.join("current"),
        });

        let plan = build_plan(&config, &State::default()).unwrap();

        assert!(matches!(
            plan.iter().find(|step| matches!(step, PlanStep::SymlinkRemove { .. })),
            Some(PlanStep::SymlinkRemove { target, .. }) if target == &target_root.join("old")
        ));
    }
}
