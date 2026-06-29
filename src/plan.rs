use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use anyhow::Result;

use crate::config::Config;
use crate::package::{PackageProvider, PackageResource, package_installed};
use crate::state::{State, StateResource};
use crate::symlink::{
    SymlinkResource, resolve_symlink_target, same_path, state_symlink, symlink_id_for,
    symlink_matches,
};

#[derive(Debug, Clone)]
pub(crate) enum PlanStep {
    SymlinkCreate(SymlinkResource),
    SymlinkUpdate(SymlinkResource),
    SymlinkRemove {
        target: PathBuf,
        source: PathBuf,
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

    for resource in &config.symlinks {
        let id = symlink_id_for(resource);
        declared.insert(id.clone());
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
        if package_installed(provider, resource)? {
            plan.push(PlanStep::PackageNoop(resource.clone()));
        } else {
            plan.push(PlanStep::PackageCreate {
                resource: resource.clone(),
                provider: provider.clone(),
            });
        }
    }

    for (id, resource) in &state.resources {
        if declared.contains(id) {
            continue;
        }
        match resource {
            StateResource::Symlink { target, source } => plan.push(PlanStep::SymlinkRemove {
                target: target.clone(),
                source: source.clone(),
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
        }
    }

    Ok(plan)
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
