use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};

use anyhow::{Context, Result, bail};

use crate::state::{State, StateResource};
use crate::symlink::home_dir;

#[derive(Debug, Clone)]
pub(crate) struct FontResource {
    pub(crate) source: PathBuf,
    pub(crate) target: PathBuf,
}

pub(crate) fn expand_font_source(root: &Path, source: Option<&str>) -> Result<Vec<FontResource>> {
    let source = source.unwrap_or("fonts");
    let source = resolve_source(root, source);
    if !source.exists() {
        return Ok(Vec::new());
    }
    if !source.is_dir() {
        bail!("font source is not a directory: {}", source.display());
    }

    let target_root = font_target_root()?;
    let mut fonts = Vec::new();
    collect_fonts(&source, &source, &target_root, &mut fonts)?;
    Ok(fonts)
}

fn collect_fonts(
    source_root: &Path,
    dir: &Path,
    target_root: &Path,
    fonts: &mut Vec<FontResource>,
) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            collect_fonts(source_root, &path, target_root, fonts)?;
            continue;
        }
        if !metadata.is_file() || !is_font_file(&path) {
            continue;
        }
        let relative = path.strip_prefix(source_root)?.to_path_buf();
        fonts.push(FontResource {
            source: path,
            target: target_root.join(relative),
        });
    }
    Ok(())
}

fn resolve_source(root: &Path, source: &str) -> PathBuf {
    let path = PathBuf::from(source);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn font_target_root() -> Result<PathBuf> {
    match std::env::consts::OS {
        "linux" => Ok(home_dir().join(".local/share/fonts/dots")),
        "macos" => Ok(home_dir().join("Library/Fonts/dots")),
        os => bail!("unsupported font platform: {os}"),
    }
}

fn is_font_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "ttf" | "otf" | "ttc" | "otc"
            )
        })
        .unwrap_or(false)
}

pub(crate) fn font_matches(resource: &FontResource) -> Result<bool> {
    same_file_contents(&resource.source, &resource.target)
}

fn same_file_contents(left: &Path, right: &Path) -> Result<bool> {
    let Ok(left) = fs::read(left) else {
        return Ok(false);
    };
    let Ok(right) = fs::read(right) else {
        return Ok(false);
    };
    Ok(left == right)
}

pub(crate) fn state_font(resource: &FontResource) -> StateResource {
    StateResource::Font {
        source: resource.source.clone(),
        target: resource.target.clone(),
    }
}

pub(crate) fn font_id_for(resource: &FontResource) -> String {
    format!("font:{}", resource.target.display())
}

pub(crate) fn apply_font(resource: &FontResource, state: &mut State) -> Result<()> {
    if let Some(parent) = resource.target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&resource.source, &resource.target).with_context(|| {
        format!(
            "failed to copy font {} -> {}",
            resource.source.display(),
            resource.target.display()
        )
    })?;
    state
        .resources
        .insert(font_id_for(resource), state_font(resource));
    Ok(())
}

pub(crate) fn remove_font(resource: &StateResource, state: &mut State) -> Result<()> {
    let StateResource::Font { target, .. } = resource else {
        return Ok(());
    };
    if target.exists() {
        fs::remove_file(target)?;
    }
    state
        .resources
        .remove(&format!("font:{}", target.display()));
    Ok(())
}

pub(crate) fn refresh_font_cache() -> Result<()> {
    if std::env::consts::OS != "linux" {
        return Ok(());
    }
    let root = font_target_root()?;
    if !root.exists() {
        return Ok(());
    }
    let status = ProcessCommand::new("fc-cache")
        .arg("-f")
        .arg(&root)
        .stdin(Stdio::null())
        .status()
        .with_context(|| "failed to run fc-cache")?;
    if !status.success() {
        bail!("fc-cache failed");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_font_extensions() {
        assert!(is_font_file(Path::new("font.ttf")));
        assert!(is_font_file(Path::new("font.OTF")));
        assert!(!is_font_file(Path::new("font.txt")));
    }
}
