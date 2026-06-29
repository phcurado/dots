use std::process::{Command as ProcessCommand, Stdio};

use anyhow::Result;

#[derive(Debug, Clone)]
pub(crate) struct PackageResource {
    pub(crate) provider: String,
    pub(crate) name: String,
}

#[derive(Debug, Clone)]
pub(crate) struct PackageProvider {
    pub(crate) available: String,
    pub(crate) installed: String,
    pub(crate) install: String,
    pub(crate) remove: String,
}

pub(crate) fn package_provider_available(provider: &PackageProvider) -> Result<bool> {
    run_provider_command(&provider.available, None, true)
}

pub(crate) fn package_installed(
    provider: &PackageProvider,
    resource: &PackageResource,
) -> Result<bool> {
    run_provider_command(&provider.installed, Some(&resource.name), true)
}

pub(crate) fn run_provider_command(
    command: &str,
    package: Option<&str>,
    quiet: bool,
) -> Result<bool> {
    let mut process = ProcessCommand::new("sh");
    process.arg("-c").arg(command);
    process.stdin(Stdio::null());
    if let Some(package) = package {
        process.env("DOTS_PACKAGE", package);
    }
    if quiet {
        process.stdout(Stdio::null()).stderr(Stdio::null());
    }
    Ok(process.status()?.success())
}
