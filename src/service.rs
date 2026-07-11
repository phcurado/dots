use std::collections::{BTreeMap, BTreeSet};
use std::process::{Command as ProcessCommand, Stdio};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub(crate) struct ServiceResource {
    pub(crate) provider: String,
    pub(crate) action: ServiceAction,
    pub(crate) name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ServiceAction {
    Start,
    Enable,
}

impl ServiceAction {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Enable => "enable",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ServiceProvider {
    pub(crate) capability: String,
    pub(crate) available: String,
    pub(crate) started: Option<String>,
    pub(crate) start: Option<String>,
    pub(crate) stop: Option<String>,
    pub(crate) enabled: Option<String>,
    pub(crate) enable: Option<String>,
    pub(crate) disable: Option<String>,
    pub(crate) list_started: Option<String>,
    pub(crate) list_enabled: Option<String>,
}

impl ServiceProvider {
    pub(crate) fn capability_name(&self) -> String {
        self.capability
            .strip_prefix("provider:")
            .unwrap_or(&self.capability)
            .to_string()
    }
}

pub(crate) fn service_provider_available(provider: &ServiceProvider) -> Result<bool> {
    run_service_provider_command(&provider.available, true)
}

pub(crate) fn service_current(
    provider: &ServiceProvider,
    resource: &ServiceResource,
) -> Result<bool> {
    let command = match resource.action {
        ServiceAction::Start => provider.started.as_deref(),
        ServiceAction::Enable => provider.enabled.as_deref(),
    }
    .ok_or_else(|| unsupported_action(resource))?;

    run_service_command(command, &resource.name, true)
}

#[derive(Debug, Default)]
pub(crate) struct ServiceStatusCache {
    providers: BTreeMap<String, Option<ServiceStatus>>,
}

#[derive(Debug, Clone, Default)]
struct ServiceStatus {
    started: Option<BTreeSet<String>>,
    enabled: Option<BTreeSet<String>>,
}

pub(crate) fn service_current_cached(
    cache: &mut ServiceStatusCache,
    provider: &ServiceProvider,
    resource: &ServiceResource,
) -> Result<bool> {
    if let Some(status) = status_for_provider(cache, &resource.provider, provider)? {
        match resource.action {
            ServiceAction::Start => {
                if let Some(started) = status.started {
                    return Ok(started.contains(&resource.name));
                }
            }
            ServiceAction::Enable => {
                if let Some(enabled) = status.enabled {
                    return Ok(enabled.contains(&resource.name));
                }
            }
        }
    }
    service_current(provider, resource)
}

fn status_for_provider(
    cache: &mut ServiceStatusCache,
    provider_name: &str,
    provider: &ServiceProvider,
) -> Result<Option<ServiceStatus>> {
    if let Some(status) = cache.providers.get(provider_name) {
        return Ok(status.clone());
    }
    let status = list_services(provider)?;
    cache
        .providers
        .insert(provider_name.to_string(), status.clone());
    Ok(status)
}

fn list_services(provider: &ServiceProvider) -> Result<Option<ServiceStatus>> {
    if provider.list_started.is_none() && provider.list_enabled.is_none() {
        return Ok(None);
    }

    Ok(Some(ServiceStatus {
        started: provider
            .list_started
            .as_deref()
            .map(command_lines)
            .transpose()?
            .flatten()
            .map(|lines| lines.into_iter().collect()),
        enabled: provider
            .list_enabled
            .as_deref()
            .map(command_lines)
            .transpose()?
            .flatten()
            .map(|lines| lines.into_iter().collect()),
    }))
}

fn command_lines(command: &str) -> Result<Option<Vec<String>>> {
    let output = ProcessCommand::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(Some(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect(),
    ))
}

pub(crate) fn service_apply(provider: &ServiceProvider, resource: &ServiceResource) -> Result<()> {
    let command = match resource.action {
        ServiceAction::Start => provider.start.as_deref(),
        ServiceAction::Enable => provider.enable.as_deref(),
    }
    .ok_or_else(|| unsupported_action(resource))?;

    if !run_service_command(command, &resource.name, false)? {
        bail!(
            "{} failed to {} {}",
            resource.provider,
            resource.action.as_str(),
            resource.name
        );
    }
    Ok(())
}

pub(crate) fn service_remove(provider: &ServiceProvider, resource: &ServiceResource) -> Result<()> {
    let command = match resource.action {
        ServiceAction::Start => provider.stop.as_deref(),
        ServiceAction::Enable => provider.disable.as_deref(),
    }
    .ok_or_else(|| unsupported_action(resource))?;

    if !run_service_command(command, &resource.name, false)? {
        bail!(
            "{} failed to remove {} {}",
            resource.provider,
            resource.action.as_str(),
            resource.name
        );
    }
    Ok(())
}

fn run_service_provider_command(command: &str, quiet: bool) -> Result<bool> {
    let mut process = ProcessCommand::new("sh");
    process.arg("-c").arg(command);
    process.stdin(Stdio::null());
    if quiet {
        process.stdout(Stdio::null()).stderr(Stdio::null());
    }
    Ok(process.status()?.success())
}

fn run_service_command(command: &str, service: &str, quiet: bool) -> Result<bool> {
    let mut process = ProcessCommand::new("sh");
    process.arg("-c").arg(command);
    process.stdin(Stdio::null()).env("DOTS_SERVICE", service);
    if quiet {
        process.stdout(Stdio::null()).stderr(Stdio::null());
    }
    Ok(process.status()?.success())
}

fn unsupported_action(resource: &ServiceResource) -> anyhow::Error {
    anyhow::anyhow!(
        "{} does not support service action {}",
        resource.provider,
        resource.action.as_str()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider() -> ServiceProvider {
        ServiceProvider {
            capability: "provider:fake".to_string(),
            available: "exit 0".to_string(),
            started: Some("exit 1".to_string()),
            start: Some("exit 0".to_string()),
            stop: Some("exit 0".to_string()),
            enabled: Some("exit 1".to_string()),
            enable: Some("exit 0".to_string()),
            disable: Some("exit 0".to_string()),
            list_started: None,
            list_enabled: None,
        }
    }

    #[test]
    fn current_status_uses_started_list() {
        let mut provider = provider();
        provider.list_started =
            Some("printf '%s\\n' docker.service tailscaled.service".to_string());
        let resource = ServiceResource {
            provider: "fake".to_string(),
            action: ServiceAction::Start,
            name: "docker.service".to_string(),
        };

        assert!(
            service_current_cached(&mut ServiceStatusCache::default(), &provider, &resource)
                .unwrap()
        );
    }

    #[test]
    fn active_enabled_timer_is_found_in_provider_inventories() {
        let mut provider = provider();
        provider.list_started = Some("printf '%s\\n' automatic-timezone.timer".to_string());
        provider.list_enabled = Some("printf '%s\\n' automatic-timezone.timer".to_string());
        let started = ServiceResource {
            provider: "fake".to_string(),
            action: ServiceAction::Start,
            name: "automatic-timezone.timer".to_string(),
        };
        let enabled = ServiceResource {
            action: ServiceAction::Enable,
            ..started.clone()
        };
        let mut cache = ServiceStatusCache::default();

        assert!(service_current_cached(&mut cache, &provider, &started).unwrap());
        assert!(service_current_cached(&mut cache, &provider, &enabled).unwrap());
    }

    #[test]
    fn current_status_uses_enabled_list() {
        let mut provider = provider();
        provider.list_enabled =
            Some("printf '%s\\n' docker.service postgresql.service".to_string());
        let resource = ServiceResource {
            provider: "fake".to_string(),
            action: ServiceAction::Enable,
            name: "postgresql.service".to_string(),
        };

        assert!(
            service_current_cached(&mut ServiceStatusCache::default(), &provider, &resource)
                .unwrap()
        );
    }
}
