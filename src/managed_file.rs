use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

use crate::state::{State, StateResource};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileResource {
    pub(crate) target: PathBuf,
    pub(crate) source: PathBuf,
    pub(crate) mode: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileStatus {
    Missing,
    Current,
    ContentChanged,
    ModeChanged,
    ContentAndModeChanged,
    Conflict,
}

pub(crate) fn file_id_for(resource: &FileResource) -> String {
    format!("file:{}", resource.target.display())
}

pub(crate) fn source_digest(resource: &FileResource) -> Result<String> {
    digest_file(&resource.source)
}

pub(crate) fn state_file(resource: &FileResource) -> Result<StateResource> {
    Ok(StateResource::File {
        target: resource.target.clone(),
        source: resource.source.clone(),
        source_digest: source_digest(resource)?,
        mode: resource.mode,
    })
}

pub(crate) fn inspect_file(resource: &FileResource) -> Result<FileStatus> {
    match fs::symlink_metadata(&resource.source) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => bail!(
            "file source is not a regular file: {}",
            resource.source.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(FileStatus::Missing);
        }
        Err(error) => return Err(error.into()),
    }

    let target_meta = match fs::symlink_metadata(&resource.target) {
        Ok(metadata) if metadata.is_file() => metadata,
        Ok(_) => return Ok(FileStatus::Conflict),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(FileStatus::Missing);
        }
        Err(error) => return Err(error.into()),
    };

    let content_matches = digest_file(&resource.source)? == digest_file(&resource.target)?;
    let mode_matches = resource
        .mode
        .map(|mode| target_meta.mode() & 0o7777 == mode)
        .unwrap_or(true);
    Ok(match (content_matches, mode_matches) {
        (true, true) => FileStatus::Current,
        (false, true) => FileStatus::ContentChanged,
        (true, false) => FileStatus::ModeChanged,
        (false, false) => FileStatus::ContentAndModeChanged,
    })
}

pub(crate) fn apply_file(resource: &FileResource, state: &mut State) -> Result<()> {
    match fs::symlink_metadata(&resource.target) {
        Ok(metadata) if !metadata.is_file() => bail!(
            "refusing to replace non-regular file: {}",
            resource.target.display()
        ),
        Ok(_) if !state.resources.contains_key(&file_id_for(resource)) => bail!(
            "refusing to replace unmanaged file: {}",
            resource.target.display()
        ),
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    let contents = fs::read(&resource.source)
        .with_context(|| format!("failed to read {}", resource.source.display()))?;
    let existing_mode = fs::symlink_metadata(&resource.target)
        .ok()
        .filter(|metadata| metadata.is_file())
        .map(|metadata| metadata.mode() & 0o7777);
    write_file_atomically(&resource.target, &contents, resource.mode.or(existing_mode))?;

    state
        .resources
        .insert(file_id_for(resource), state_file(resource)?);
    Ok(())
}

pub(crate) fn apply_file_mode(resource: &FileResource, state: &mut State) -> Result<()> {
    let mode = resource.mode.expect("mode update requires a declared mode");
    ensure_mode(&resource.target, mode)?;
    state
        .resources
        .insert(file_id_for(resource), state_file(resource)?);
    Ok(())
}

pub(crate) fn digest_file(path: &Path) -> Result<String> {
    let contents = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let digest = Sha256::digest(contents);
    Ok(format!("sha256:{digest:x}"))
}

pub(crate) fn ensure_mode(path: &Path, mode: u32) -> Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .with_context(|| format!("failed to set mode on {}", path.display()))?;
    Ok(())
}

pub(crate) fn write_file_atomically(
    target: &Path,
    contents: &[u8],
    mode: Option<u32>,
) -> Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let temporary = temporary_path(target)?;
    let result = (|| -> Result<()> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        if let Some(mode) = mode {
            options.mode(mode);
        }
        let mut file = options
            .open(&temporary)
            .with_context(|| format!("failed to create {}", temporary.display()))?;
        if let Some(mode) = mode {
            ensure_mode(&temporary, mode)?;
        }
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, target).with_context(|| {
            format!(
                "failed to replace {} with {}",
                target.display(),
                temporary.display()
            )
        })?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn temporary_path(target: &Path) -> Result<PathBuf> {
    let parent = target.parent().context("file target has no parent")?;
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .context("file target has no valid file name")?;
    for _ in 0..100 {
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(".{name}.dots-{}-{counter}.tmp", process::id()));
        if !path.exists() {
            return Ok(path);
        }
    }
    bail!("could not allocate temporary file for {}", target.display())
}
