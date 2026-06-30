use std::process::{Command as ProcessCommand, Stdio};

use anyhow::{Result, bail};

#[derive(Debug, Clone)]
pub(crate) struct CommandResource {
    pub(crate) name: String,
    pub(crate) check: String,
    pub(crate) apply: String,
    pub(crate) needs: Vec<String>,
    pub(crate) provides: Vec<String>,
}

pub(crate) fn command_id_for(resource: &CommandResource) -> String {
    format!("command:{}", resource.name)
}

pub(crate) fn command_current(resource: &CommandResource) -> Result<bool> {
    run_shell(&resource.check, true)
}

pub(crate) fn command_apply(resource: &CommandResource) -> Result<()> {
    if !run_shell(&resource.apply, false)? {
        bail!("command {} failed", resource.name);
    }
    if !command_current(resource)? {
        bail!(
            "command {} did not pass its check after apply",
            resource.name
        );
    }
    Ok(())
}

fn run_shell(command: &str, quiet: bool) -> Result<bool> {
    let mut process = ProcessCommand::new("sh");
    process.arg("-c").arg(command);
    if quiet {
        process
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
    }
    Ok(process.status()?.success())
}
