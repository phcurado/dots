use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::state::StateResource;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComposeResource {
    pub(crate) name: String,
    pub(crate) file: PathBuf,
    pub(crate) profiles: Vec<String>,
    pub(crate) apply: Vec<String>,
    pub(crate) remove: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ComposeContainer {
    service: String,
    state: String,
    health: String,
}

pub(crate) fn compose_id_for(resource: &ComposeResource) -> String {
    format!("compose:{}", resource.name)
}

pub(crate) fn compose_available() -> Result<bool> {
    Ok(Command::new("docker")
        .args(["compose", "version"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false))
}

pub(crate) fn compose_current(
    resource: &ComposeResource,
    fingerprint: Option<&str>,
) -> Result<bool> {
    let desired_fingerprint = compose_fingerprint(resource)?;
    if fingerprint != Some(desired_fingerprint.as_str()) {
        return Ok(false);
    }

    let services = compose_output(resource, &["config", "--services"])?;
    if !services.status.success() {
        return Ok(false);
    }
    let services_output = String::from_utf8_lossy(&services.stdout);
    let expected = services_output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if expected.is_empty() {
        return Ok(false);
    }

    let output = compose_output(resource, &["ps", "--all", "--format", "json"])?;
    if !output.status.success() {
        return Ok(false);
    }
    let source = String::from_utf8_lossy(&output.stdout);
    let containers = parse_containers(&source)?;

    Ok(expected.iter().all(|service| {
        containers.iter().any(|container| {
            container.service == *service
                && container.state == "running"
                && (container.health.is_empty() || container.health == "healthy")
        })
    }))
}

pub(crate) fn compose_apply(resource: &ComposeResource) -> Result<String> {
    let status = compose_command(resource)
        .args(&resource.apply)
        .status()
        .with_context(|| "failed to run docker compose")?;
    if !status.success() {
        bail!("docker compose failed to apply {}", resource.name);
    }
    let fingerprint = compose_fingerprint(resource)?;
    if !compose_current(resource, Some(&fingerprint))? {
        bail!(
            "docker compose {} is not running after apply",
            resource.name
        );
    }
    Ok(fingerprint)
}

pub(crate) fn compose_remove(resource: &ComposeResource, stored_config: &str) -> Result<()> {
    let status = if resource.file.exists() {
        compose_command(resource)
            .args(&resource.remove)
            .status()
            .with_context(|| "failed to run docker compose")?
    } else {
        let mut child = compose_command_with_file(resource, Path::new("-"))
            .args(&resource.remove)
            .stdin(Stdio::piped())
            .spawn()
            .with_context(|| "failed to run docker compose")?;
        child
            .stdin
            .take()
            .expect("compose stdin is piped")
            .write_all(stored_config.as_bytes())?;
        child.wait()?
    };
    if !status.success() {
        bail!("docker compose failed to remove {}", resource.name);
    }
    Ok(())
}

pub(crate) fn state_compose(resource: &ComposeResource, fingerprint: String) -> StateResource {
    StateResource::Compose {
        name: resource.name.clone(),
        file: resource.file.clone(),
        profiles: resource.profiles.clone(),
        apply: resource.apply.clone(),
        remove: resource.remove.clone(),
        fingerprint,
    }
}

pub(crate) fn compose_from_state(resource: &StateResource) -> Option<(ComposeResource, &str)> {
    let StateResource::Compose {
        name,
        file,
        profiles,
        apply,
        remove,
        fingerprint,
    } = resource
    else {
        return None;
    };
    Some((
        ComposeResource {
            name: name.clone(),
            file: file.clone(),
            profiles: profiles.clone(),
            apply: apply.clone(),
            remove: remove.clone(),
        },
        fingerprint,
    ))
}

fn compose_fingerprint(resource: &ComposeResource) -> Result<String> {
    let output = compose_output(resource, &["config"])?;
    if !output.status.success() {
        bail!(
            "invalid Docker Compose configuration: {}",
            resource.file.display()
        );
    }
    String::from_utf8(output.stdout).context("docker compose config returned invalid UTF-8")
}

fn compose_output(resource: &ComposeResource, args: &[&str]) -> Result<Output> {
    compose_command(resource)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .with_context(|| "failed to run docker compose")
}

fn compose_command(resource: &ComposeResource) -> Command {
    compose_command_with_file(resource, &resource.file)
}

fn compose_command_with_file(resource: &ComposeResource, file: &Path) -> Command {
    let mut command = Command::new("docker");
    command
        .arg("compose")
        .arg("--project-name")
        .arg(&resource.name)
        .arg("--file")
        .arg(file);
    for profile in &resource.profiles {
        command.arg("--profile").arg(profile);
    }
    command
}

fn parse_containers(source: &str) -> Result<Vec<ComposeContainer>> {
    let source = source.trim();
    if source.is_empty() {
        return Ok(Vec::new());
    }
    if source.starts_with('[') {
        return serde_json::from_str(source).context("failed to parse docker compose ps output");
    }
    source
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).context("failed to parse docker compose ps output"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_lines_ps_output() {
        let containers = parse_containers(
            r#"{"Service":"web","State":"running","Health":"healthy"}
{"Service":"db","State":"exited","Health":""}"#,
        )
        .unwrap();

        assert_eq!(containers.len(), 2);
        assert_eq!(containers[0].service, "web");
        assert_eq!(containers[1].state, "exited");
    }

    #[test]
    fn parses_json_array_ps_output() {
        let containers =
            parse_containers(r#"[{"Service":"web","State":"running","Health":""}]"#).unwrap();

        assert_eq!(containers.len(), 1);
    }
}
