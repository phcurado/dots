use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::{Command as ProcessCommand, Stdio};

use anyhow::Result;

use crate::command::{CommandResource, command_current};
use crate::config::Config;
use crate::font::{FontResource, font_matches};
use crate::package::{
    PackageProvider, PackageResource, PackageStatusCache, package_installed_cached,
    package_provider_available,
};
use crate::service::{
    ServiceProvider, ServiceResource, ServiceStatusCache, service_current_cached,
};
use crate::state::{State, StateResource};
use crate::symlink::{
    SymlinkResource, regular_file_matches, resolve_symlink_target, same_path,
    stale_symlinks_for_declaration, state_symlink, symlink_id_for, symlink_matches,
};
use crate::user::{
    UserGroupResource, UserShellResource, current_shell, group_exists, shell_matches,
};

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
        reason: String,
    },
    PackageCreate {
        resource: PackageResource,
        provider: PackageProvider,
    },
    PackageRemove {
        resource: PackageResource,
        provider: PackageProvider,
    },
    PackageNoop(PackageResource),
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
    UserShellUpdate {
        resource: UserShellResource,
        current: Option<PathBuf>,
    },
    UserShellNoop,
    UserGroupAdd(UserGroupResource),
    UserGroupNoop,
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

pub(crate) fn refresh_state_from_system(config: &Config, state: &mut State) -> Result<()> {
    for resource in &config.symlinks {
        if symlink_matches(resource)? {
            state
                .resources
                .insert(symlink_id_for(resource), state_symlink(resource));
        }
    }

    Ok(())
}

pub(crate) fn build_plan(config: &Config, state: &State) -> Result<Vec<PlanStep>> {
    let mut plan = Vec::new();
    let mut declared = BTreeSet::new();
    let mut declared_symlink_targets = BTreeSet::new();
    let mut planned_symlink_removals = BTreeSet::new();

    for resource in &config.symlinks {
        let id = symlink_id_for(resource);
        declared.insert(id.clone());
        declared_symlink_targets.insert(resource.target.clone());
        let owned = state.resources.contains_key(&id);

        if !resource.source.exists() {
            plan.push(PlanStep::SymlinkConflict {
                resource: resource.clone(),
                reason: format!("source does not exist: {}", resource.source.display()),
            });
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
                        reason: "target exists but is not managed".to_string(),
                    });
                }
            }
            Ok(meta) if meta.is_file() && regular_file_matches(resource)? => {
                plan.push(PlanStep::SymlinkUpdate(resource.clone()));
            }
            Ok(_) => plan.push(PlanStep::SymlinkConflict {
                resource: resource.clone(),
                reason: "target exists and is not a symlink".to_string(),
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
        provided_capabilities.extend(package_provides(resource));
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
        let capability = provider_capability(&resource.provider);
        if !provided_capabilities.contains(&capability) && !package_provider_available(provider)? {
            if missing_capabilities.insert(capability.clone()) {
                plan.push(PlanStep::CapabilityConflict {
                    capability: capability_name(&capability),
                    reason: "is not available".to_string(),
                });
            }
            continue;
        }
        if package_installed_cached(&mut package_status, provider, resource)? {
            plan.push(PlanStep::PackageNoop(resource.clone()));
        } else {
            plan.push(PlanStep::PackageCreate {
                resource: resource.clone(),
                provider: provider.clone(),
            });
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
        let capability = provider_capability(&resource.provider);
        if !provided_capabilities.contains(&capability) && !capability_available(&capability)? {
            if missing_capabilities.insert(capability.clone()) {
                plan.push(PlanStep::CapabilityConflict {
                    capability: capability_name(&capability),
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

    for resource in &config.user.groups {
        if std::env::consts::OS != "linux" {
            plan.push(PlanStep::UserGroupConflict {
                resource: resource.clone(),
                reason: "user groups are only supported on Linux".to_string(),
            });
            continue;
        }
        if group_exists(resource)? {
            plan.push(PlanStep::UserGroupNoop);
        } else {
            plan.push(PlanStep::UserGroupAdd(resource.clone()));
        }
    }

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
            StateResource::Package { provider, name } => {
                let resource = PackageResource {
                    provider: provider.clone(),
                    name: name.clone(),
                };
                match config.package_providers.get(provider) {
                    Some(provider) => plan.push(PlanStep::PackageRemove {
                        resource,
                        provider: provider.clone(),
                    }),
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
                let action = match action.as_str() {
                    "start" => crate::service::ServiceAction::Start,
                    "enable" => crate::service::ServiceAction::Enable,
                    _ => continue,
                };
                let resource = ServiceResource {
                    provider: provider.clone(),
                    action,
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
        }
    }

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

fn package_provides(resource: &PackageResource) -> Vec<String> {
    match (resource.provider.as_str(), resource.name.as_str()) {
        ("pacman", "paru") => vec!["provider:paru".to_string()],
        _ => Vec::new(),
    }
}

fn provider_capability(provider: &str) -> String {
    match provider {
        "brew" | "brew-cask" | "brew-tap" | "brew-service" => "provider:brew".to_string(),
        provider => format!("provider:{provider}"),
    }
}

fn capability_name(capability: &str) -> String {
    capability
        .strip_prefix("provider:")
        .unwrap_or(capability)
        .to_string()
}

fn capability_available(capability: &str) -> Result<bool> {
    let command = match capability {
        "provider:brew" => "command -v brew >/dev/null",
        "provider:systemd" => "command -v systemctl >/dev/null",
        _ => return Ok(true),
    };
    Ok(ProcessCommand::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?
        .success())
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
        action: resource.action.as_str().to_string(),
        name: resource.name.clone(),
    }
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

    fn fake_provider(available: &str, installed: &str) -> PackageProvider {
        PackageProvider {
            available: available.to_string(),
            installed: installed.to_string(),
            install: "exit 0".to_string(),
            remove: "exit 0".to_string(),
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
    fn installed_packages_are_not_auto_tracked_by_refresh() {
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

        assert!(state.resources.is_empty());
        assert!(matches!(plan.as_slice(), [PlanStep::PackageNoop(_)]));
    }

    #[test]
    fn identical_regular_symlink_target_can_be_replaced() {
        let root =
            std::env::temp_dir().join(format!("dots-identical-target-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let target = root.join("target");
        let source = root.join("source");
        fs::write(&target, "same").unwrap();
        fs::write(&source, "same").unwrap();

        let mut config = Config::default();
        config.symlinks.push(SymlinkResource { target, source });

        let plan = build_plan(&config, &State::default()).unwrap();

        assert!(matches!(plan.as_slice(), [PlanStep::SymlinkUpdate(_)]));
    }

    #[test]
    fn untracked_stale_symlinks_under_managed_directory_are_removed() {
        let root = std::env::temp_dir().join(format!("dots-stale-link-{}", std::process::id()));
        let source_root = root.join("source");
        let target_root = root.join("target");
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
