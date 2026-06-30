use std::collections::{BTreeMap, BTreeSet};
use std::process::{Command as ProcessCommand, Stdio};

use anyhow::{Result, bail};

#[derive(Debug, Clone)]
pub(crate) struct ServiceResource {
    pub(crate) provider: String,
    pub(crate) action: ServiceAction,
    pub(crate) name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub(crate) started: Option<String>,
    pub(crate) start: Option<String>,
    pub(crate) stop: Option<String>,
    pub(crate) enabled: Option<String>,
    pub(crate) enable: Option<String>,
    pub(crate) disable: Option<String>,
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
    started: BTreeSet<String>,
    enabled: BTreeSet<String>,
}

pub(crate) fn service_current_cached(
    cache: &mut ServiceStatusCache,
    provider: &ServiceProvider,
    resource: &ServiceResource,
) -> Result<bool> {
    if let Some(status) = status_for_provider(cache, &resource.provider)? {
        return Ok(match resource.action {
            ServiceAction::Start => status.started.contains(&resource.name),
            ServiceAction::Enable => status.enabled.contains(&resource.name),
        });
    }
    service_current(provider, resource)
}

fn status_for_provider(
    cache: &mut ServiceStatusCache,
    provider: &str,
) -> Result<Option<ServiceStatus>> {
    if let Some(status) = cache.providers.get(provider) {
        return Ok(status.clone());
    }
    let status = list_services(provider)?;
    cache.providers.insert(provider.to_string(), status.clone());
    Ok(status)
}

fn list_services(provider: &str) -> Result<Option<ServiceStatus>> {
    match provider {
        "systemd" => list_systemd_services(),
        "brew-service" => list_brew_services(),
        _ => Ok(None),
    }
}

fn list_systemd_services() -> Result<Option<ServiceStatus>> {
    let started = command_lines(
        "systemctl list-units --type=service --state=running --no-legend --no-pager",
    )?;
    let unit_files =
        command_lines("systemctl list-unit-files --type=service --no-legend --no-pager")?;
    let Some(started) = started else {
        return Ok(None);
    };
    let Some(unit_files) = unit_files else {
        return Ok(None);
    };

    let started = started
        .iter()
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_string)
        .collect();
    let enabled = unit_files
        .iter()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let name = parts.next()?;
            let state = parts.next()?;
            matches!(
                state,
                "enabled" | "enabled-runtime" | "linked" | "linked-runtime"
            )
            .then(|| name.to_string())
        })
        .collect();

    Ok(Some(ServiceStatus { started, enabled }))
}

fn list_brew_services() -> Result<Option<ServiceStatus>> {
    let Some(lines) = command_lines("brew services list")? else {
        return Ok(None);
    };
    let started = lines
        .iter()
        .skip(1)
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let name = parts.next()?;
            let status = parts.next()?;
            (status == "started").then(|| name.to_string())
        })
        .collect();

    Ok(Some(ServiceStatus {
        started,
        enabled: BTreeSet::new(),
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
