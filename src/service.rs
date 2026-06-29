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
