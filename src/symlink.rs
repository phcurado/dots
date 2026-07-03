use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::state::{State, StateResource};

#[derive(Debug, Clone)]
pub(crate) struct SymlinkResource {
    pub(crate) target: PathBuf,
    pub(crate) source: PathBuf,
}

#[derive(Debug, Clone)]
pub(crate) struct SymlinkCandidate {
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

pub(crate) fn symlink_candidate_for_resource(
    resource: &SymlinkResource,
) -> Result<Option<SymlinkCandidate>> {
    if resource.source.exists() {
        return Ok(None);
    }
    let Ok(metadata) = fs::symlink_metadata(&resource.target) else {
        return Ok(None);
    };
    if metadata.file_type().is_symlink()
        || !resource
            .source
            .parent()
            .map(|parent| parent.is_dir())
            .unwrap_or(false)
    {
        return Ok(None);
    }
    Ok(Some(SymlinkCandidate {
        target: resource.target.clone(),
        source: resource.source.clone(),
    }))
}

pub(crate) fn symlink_candidate_for_target(
    declaration: &SymlinkDeclaration,
    target: &Path,
) -> Result<Option<SymlinkCandidate>> {
    let Ok(relative) = target.strip_prefix(&declaration.target) else {
        return Ok(None);
    };
    if relative.as_os_str().is_empty() {
        return Ok(None);
    }

    let ignore = build_ignore_set(&declaration.ignore)?;
    if ignore.is_match(relative) {
        return Ok(None);
    }

    let source = declaration.source.join(relative);
    if source.exists()
        || !source
            .parent()
            .map(|parent| parent.is_dir())
            .unwrap_or(false)
    {
        return Ok(None);
    }

    let Ok(metadata) = fs::symlink_metadata(target) else {
        return Ok(None);
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Ok(None);
    }

    Ok(Some(SymlinkCandidate {
        target: target.to_path_buf(),
        source,
    }))
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
            let current = normalize_path(&resolve_symlink_target(&target, &current));
            let source_root = normalize_path(source_root);
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

fn relative_path(from_dir: &Path, to: &Path) -> PathBuf {
    let from_dir = normalize_path(from_dir);
    let to = normalize_path(to);
    if from_dir.is_absolute() != to.is_absolute() {
        return to;
    }

    let from_components = path_components_without_root(&from_dir);
    let to_components = path_components_without_root(&to);
    let common = from_components
        .iter()
        .zip(&to_components)
        .take_while(|(left, right)| left == right)
        .count();

    let mut relative = PathBuf::new();
    for _ in common..from_components.len() {
        relative.push("..");
    }
    for component in &to_components[common..] {
        relative.push(component);
    }
    if relative.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        relative
    }
}

fn path_components_without_root(path: &Path) -> Vec<OsString> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(component) => Some(component.to_os_string()),
            _ => None,
        })
        .collect()
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
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

pub(crate) fn apply_symlink_candidate(
    candidate: &SymlinkCandidate,
    state: &mut State,
) -> Result<()> {
    let Some(parent) = candidate.source.parent() else {
        bail!("source path has no parent: {}", candidate.source.display());
    };
    if !parent.is_dir() {
        bail!("source parent does not exist: {}", parent.display());
    }
    if candidate.source.exists() {
        bail!("source already exists: {}", candidate.source.display());
    }

    let metadata = fs::symlink_metadata(&candidate.target)
        .with_context(|| format!("target does not exist: {}", candidate.target.display()))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        bail!(
            "target is not a regular file: {}",
            candidate.target.display()
        );
    }

    fs::rename(&candidate.target, &candidate.source).with_context(|| {
        format!(
            "failed to import {} to {}",
            candidate.target.display(),
            candidate.source.display()
        )
    })?;
    apply_symlink(
        &SymlinkResource {
            target: candidate.target.clone(),
            source: candidate.source.clone(),
        },
        state,
    )
}

pub(crate) fn apply_symlink(resource: &SymlinkResource, state: &mut State) -> Result<()> {
    if let Some(parent) = resource.target.parent() {
        fs::create_dir_all(parent)?;
    }
    ensure_safe_to_replace(resource, state)?;

    let link_target = relative_path(
        resource.target.parent().unwrap_or_else(|| Path::new(".")),
        &resource.source,
    );
    let tmp = temporary_link_path(&resource.target);
    unix_fs::symlink(&link_target, &tmp).with_context(|| {
        format!(
            "failed to symlink {} -> {}",
            tmp.display(),
            resource.source.display()
        )
    })?;
    if let Err(error) = fs::rename(&tmp, &resource.target) {
        let _ = fs::remove_file(&tmp);
        return Err(error).with_context(|| {
            format!(
                "failed to symlink {} -> {}",
                resource.target.display(),
                resource.source.display()
            )
        });
    }
    state
        .resources
        .insert(symlink_id_for(resource), state_symlink(resource));
    Ok(())
}

fn ensure_safe_to_replace(resource: &SymlinkResource, state: &State) -> Result<()> {
    let Ok(metadata) = fs::symlink_metadata(&resource.target) else {
        return Ok(());
    };

    if metadata.file_type().is_symlink() {
        let current = resolve_symlink_target(&resource.target, &fs::read_link(&resource.target)?);
        if same_path(&current, &resource.source)
            || state
                .resources
                .get(&symlink_id_for(resource))
                .is_some_and(|state_resource| match state_resource {
                    StateResource::Symlink { source, .. } => same_path(&current, source),
                    _ => false,
                })
        {
            return Ok(());
        }
    } else if metadata.is_file() && regular_file_matches(resource)? {
        return Ok(());
    }

    bail!(
        "refusing to replace unmanaged target: {}",
        resource.target.display()
    )
}

fn temporary_link_path(target: &Path) -> PathBuf {
    let name = target
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| "link".into());
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    target.with_file_name(format!(".{name}.dots-tmp-{}-{nonce}", std::process::id()))
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
        if same_path(&current, source) || normalize_path(&current) == normalize_path(source) {
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
        let base = tempfile::tempdir().unwrap();
        assert!(!same_path(
            &base.path().join("one"),
            &base.path().join("two")
        ));
    }

    #[test]
    fn apply_symlink_uses_relative_link_target() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("repo/.config/app/config.toml");
        let target = root.path().join("home/.config/app/config.toml");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, "config").unwrap();

        let resource = SymlinkResource {
            target: target.clone(),
            source: source.clone(),
        };
        apply_symlink(&resource, &mut State::default()).unwrap();

        assert_eq!(
            fs::read_link(&target).unwrap(),
            PathBuf::from("../../../repo/.config/app/config.toml")
        );
        assert!(symlink_matches(&resource).unwrap());
    }

    #[test]
    fn apply_symlink_refuses_unmanaged_target() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let target = root.path().join("target");
        fs::write(&source, "source").unwrap();
        fs::write(&target, "other").unwrap();

        let error = apply_symlink(&SymlinkResource { target, source }, &mut State::default())
            .unwrap_err()
            .to_string();

        assert!(error.contains("refusing to replace unmanaged target"));
    }

    #[test]
    fn remove_symlink_removes_link_to_missing_source() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("repo/removed");
        let target = root.path().join("home/removed");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        unix_fs::symlink(&source, &target).unwrap();

        let mut state = State::default();
        remove_symlink(
            &StateResource::Symlink {
                target: target.clone(),
                source,
            },
            &mut state,
        )
        .unwrap();

        assert!(fs::symlink_metadata(&target).is_err());
    }

    #[test]
    fn directory_expansion_ignores_git_metadata() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let target = root.path().join("target");
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
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let target = root.path().join("target");
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
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let target = root.path().join("target");
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

    #[test]
    fn stale_symlink_scan_handles_relative_links_with_parent_segments() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("repo/.config");
        let target = root.path().join("home/.config");
        fs::create_dir_all(source.join("nvim")).unwrap();
        fs::create_dir_all(target.join("nvim/removed-plugin")).unwrap();
        let stale = target.join("nvim/removed-plugin/init.lua");
        unix_fs::symlink(
            "../../../../repo/.config/nvim/removed-plugin/init.lua",
            &stale,
        )
        .unwrap();

        let resources = stale_symlinks_for_declaration(
            &SymlinkDeclaration {
                target,
                source,
                ignore: Vec::new(),
            },
            &BTreeSet::new(),
        )
        .unwrap();

        assert!(resources.iter().any(|resource| resource.target == stale));
    }
}
