use std::collections::BTreeSet;
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
    for pattern in [
        ".git",
        ".git/**",
        "**/.git",
        "**/.git/**",
        ".gitignore",
        "**/.gitignore",
        ".gitmodules",
        "**/.gitmodules",
    ] {
        builder
            .add(Glob::new(pattern).with_context(|| format!("invalid ignore pattern: {pattern}"))?);
    }
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

pub(crate) fn regular_file_matches(resource: &SymlinkResource) -> Result<bool> {
    let Ok(target_meta) = fs::symlink_metadata(&resource.target) else {
        return Ok(false);
    };
    let Ok(source_meta) = fs::symlink_metadata(&resource.source) else {
        return Ok(false);
    };
    if !target_meta.is_file() || !source_meta.is_file() {
        return Ok(false);
    }
    if target_meta.len() != source_meta.len() {
        return Ok(false);
    }
    Ok(fs::read(&resource.target)? == fs::read(&resource.source)?)
}

pub(crate) fn stale_symlinks_for_declaration(
    declaration: &SymlinkDeclaration,
    declared_targets: &BTreeSet<PathBuf>,
) -> Result<Vec<SymlinkResource>> {
    let ignore = build_ignore_set(&declaration.ignore)?;
    let mut resources = Vec::new();
    collect_stale_symlinks(
        &declaration.target,
        &declaration.source,
        Path::new(""),
        false,
        &ignore,
        declared_targets,
        &mut resources,
    )?;
    Ok(resources)
}

fn collect_stale_symlinks(
    target_root: &Path,
    source_root: &Path,
    relative: &Path,
    inside_managed_tree: bool,
    ignore: &GlobSet,
    declared_targets: &BTreeSet<PathBuf>,
    resources: &mut Vec<SymlinkResource>,
) -> Result<()> {
    let source_dir = source_root.join(relative);
    let source_dir_exists = source_dir.is_dir();
    if !source_dir_exists && !inside_managed_tree {
        return Ok(());
    }
    let current_is_root = relative.as_os_str().is_empty();
    let inside_managed_tree = inside_managed_tree || (!current_is_root && source_dir_exists);

    let target_dir = target_root.join(relative);
    let Ok(entries) = fs::read_dir(&target_dir) else {
        return Ok(());
    };

    for entry in entries {
        let entry = entry?;
        let relative = relative.join(entry.file_name());
        if ignore.is_match(&relative) {
            continue;
        }

        let target = target_root.join(&relative);
        let metadata = fs::symlink_metadata(&target)?;
        if metadata.file_type().is_symlink() {
            if declared_targets.contains(&target) {
                continue;
            }
            let current = fs::read_link(&target)?;
            let current = resolve_symlink_target(&target, &current);
            if current.starts_with(source_root) {
                resources.push(SymlinkResource {
                    target,
                    source: current,
                });
            }
        } else if metadata.is_dir() {
            let child_source_exists = source_root.join(&relative).is_dir();
            if inside_managed_tree || child_source_exists {
                collect_stale_symlinks(
                    target_root,
                    source_root,
                    &relative,
                    inside_managed_tree,
                    ignore,
                    declared_targets,
                    resources,
                )?;
            }
        }
    }

    Ok(())
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

    #[test]
    fn directory_expansion_ignores_git_metadata() {
        let root = std::env::temp_dir().join(format!("dots-git-ignore-{}", std::process::id()));
        let source = root.join("source");
        let target = root.join("target");
        fs::create_dir_all(source.join("plugin/.git")).unwrap();
        fs::create_dir_all(target.join("plugin")).unwrap();
        fs::write(source.join("plugin/.git/config"), "").unwrap();
        fs::write(source.join("plugin/.gitignore"), "").unwrap();
        fs::write(source.join("plugin/file"), "").unwrap();

        let resources = expand_symlink_declaration(&SymlinkDeclaration {
            target,
            source: source.clone(),
            ignore: Vec::new(),
        })
        .unwrap();

        assert!(
            resources
                .iter()
                .any(|resource| resource.source == source.join("plugin/file"))
        );
        assert!(resources.iter().all(|resource| {
            let source = resource.source.display().to_string();
            !source.contains(".git")
        }));
    }

    #[test]
    fn stale_symlink_scan_skips_top_level_target_dirs_missing_from_source() {
        let root = std::env::temp_dir().join(format!("dots-stale-skip-{}", std::process::id()));
        let source = root.join("source");
        let target = root.join("target");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(target.join("large/unrelated/tree")).unwrap();
        let stale = target.join("large/unrelated/tree/stale");
        unix_fs::symlink(source.join("old"), &stale).unwrap();

        let resources = stale_symlinks_for_declaration(
            &SymlinkDeclaration {
                target,
                source,
                ignore: Vec::new(),
            },
            &BTreeSet::new(),
        )
        .unwrap();

        assert!(resources.is_empty());
    }

    #[test]
    fn stale_symlink_scan_keeps_scanning_inside_managed_dirs() {
        let root = std::env::temp_dir().join(format!("dots-stale-inside-{}", std::process::id()));
        let source = root.join("source");
        let target = root.join("target");
        fs::create_dir_all(source.join("nvim")).unwrap();
        fs::create_dir_all(target.join("nvim/removed-plugin")).unwrap();
        let stale = target.join("nvim/removed-plugin/init.lua");
        unix_fs::symlink(source.join("nvim/removed-plugin/init.lua"), &stale).unwrap();

        let resources = stale_symlinks_for_declaration(
            &SymlinkDeclaration {
                target,
                source: source.clone(),
                ignore: Vec::new(),
            },
            &BTreeSet::new(),
        )
        .unwrap();

        assert!(resources.iter().any(|resource| resource.target == stale));
    }
}
