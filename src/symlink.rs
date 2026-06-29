use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::state::{State, StateResource};

#[derive(Debug, Clone)]
pub(crate) struct SymlinkResource {
    pub(crate) target: PathBuf,
    pub(crate) source: PathBuf,
}

#[derive(Debug, Clone)]
pub(crate) struct SymlinkDeclaration {
    pub(crate) target: PathBuf,
    pub(crate) source: PathBuf,
    pub(crate) ignore: Vec<String>,
}

pub(crate) fn expand_home(path: &str) -> PathBuf {
    if path == "~" {
        return home_dir();
    }
    if let Some(rest) = path.strip_prefix("~/") {
        return home_dir().join(rest);
    }
    PathBuf::from(path)
}

pub(crate) fn resolve_source(root: &Path, source: &str) -> PathBuf {
    let path = expand_home(source);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

pub(crate) fn expand_symlink_declaration(
    declaration: &SymlinkDeclaration,
) -> Result<Vec<SymlinkResource>> {
    let target_is_directory = fs::symlink_metadata(&declaration.target)
        .map(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
        .unwrap_or(false);
    let should_expand =
        declaration.source.is_dir() && (!declaration.ignore.is_empty() || target_is_directory);

    if !should_expand {
        return Ok(vec![SymlinkResource {
            target: declaration.target.clone(),
            source: declaration.source.clone(),
        }]);
    }

    let ignore = build_ignore_set(&declaration.ignore)?;
    let mut resources = Vec::new();
    expand_symlink_dir(
        &declaration.target,
        &declaration.source,
        Path::new(""),
        &ignore,
        &mut resources,
    )?;
    Ok(resources)
}

fn expand_symlink_dir(
    target_root: &Path,
    source_root: &Path,
    relative: &Path,
    ignore: &GlobSet,
    resources: &mut Vec<SymlinkResource>,
) -> Result<()> {
    let source_dir = source_root.join(relative);
    for entry in fs::read_dir(&source_dir)
        .with_context(|| format!("failed to read {}", source_dir.display()))?
    {
        let entry = entry?;
        let name = entry.file_name();
        let relative = relative.join(name);
        if ignore.is_match(&relative) {
            continue;
        }

        let source = source_root.join(&relative);
        let target = target_root.join(&relative);
        let metadata = fs::symlink_metadata(&source)?;

        if metadata.is_dir()
            && fs::symlink_metadata(&target)
                .map(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
                .unwrap_or(false)
        {
            expand_symlink_dir(target_root, source_root, &relative, ignore, resources)?;
        } else {
            resources.push(SymlinkResource { target, source });
        }
    }
    Ok(())
}

fn build_ignore_set(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder
            .add(Glob::new(pattern).with_context(|| format!("invalid ignore pattern: {pattern}"))?);
        if let Some(dir) = pattern.strip_suffix("/**") {
            builder.add(Glob::new(dir).with_context(|| format!("invalid ignore pattern: {dir}"))?);
        }
    }
    Ok(builder.build()?)
}

pub(crate) fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("~"))
}

pub(crate) fn same_path(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

pub(crate) fn symlink_matches(resource: &SymlinkResource) -> Result<bool> {
    let Ok(meta) = fs::symlink_metadata(&resource.target) else {
        return Ok(false);
    };
    if !meta.file_type().is_symlink() || !resource.source.exists() {
        return Ok(false);
    }
    let current = fs::read_link(&resource.target)?;
    let current = resolve_symlink_target(&resource.target, &current);
    Ok(same_path(&current, &resource.source))
}

pub(crate) fn resolve_symlink_target(target: &Path, link: &Path) -> PathBuf {
    if link.is_absolute() {
        link.to_path_buf()
    } else {
        target.parent().unwrap_or_else(|| Path::new(".")).join(link)
    }
}

pub(crate) fn state_symlink(resource: &SymlinkResource) -> StateResource {
    StateResource::Symlink {
        target: resource.target.clone(),
        source: resource.source.clone(),
    }
}

pub(crate) fn symlink_id_for(resource: &SymlinkResource) -> String {
    format!("symlink:{}", resource.target.display())
}

pub(crate) fn apply_symlink(resource: &SymlinkResource, state: &mut State) -> Result<()> {
    if let Some(parent) = resource.target.parent() {
        fs::create_dir_all(parent)?;
    }
    if resource.target.exists() || fs::symlink_metadata(&resource.target).is_ok() {
        fs::remove_file(&resource.target)?;
    }
    unix_fs::symlink(&resource.source, &resource.target).with_context(|| {
        format!(
            "failed to symlink {} -> {}",
            resource.target.display(),
            resource.source.display()
        )
    })?;
    state
        .resources
        .insert(symlink_id_for(resource), state_symlink(resource));
    Ok(())
}

pub(crate) fn remove_symlink(resource: &StateResource, state: &mut State) -> Result<()> {
    let StateResource::Symlink { target, source } = resource else {
        return Ok(());
    };
    if fs::symlink_metadata(target)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
    {
        let current = fs::read_link(target)?;
        let current = resolve_symlink_target(target, &current);
        if same_path(&current, source) {
            fs::remove_file(target)?;
        }
    }
    state
        .resources
        .remove(&format!("symlink:{}", target.display()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_paths_are_not_the_same_path() {
        let base = std::env::temp_dir().join(format!("dots-missing-{}", std::process::id()));
        assert!(!same_path(&base.join("one"), &base.join("two")));
    }
}
