use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};

#[derive(Debug)]
pub(crate) struct Project {
    pub(crate) root: PathBuf,
    pub(crate) config: PathBuf,
}

pub(crate) fn find_project(file: Option<PathBuf>) -> Result<Project> {
    if let Some(config) = file {
        let config = fs::canonicalize(&config)
            .with_context(|| format!("failed to resolve {}", config.display()))?;
        let root = config
            .parent()
            .context("config path has no parent")?
            .to_path_buf();
        return Ok(Project { root, config });
    }

    let mut dir = std::env::current_dir()?;
    loop {
        let config = dir.join("dots.lua");
        if config.exists() {
            return Ok(Project { root: dir, config });
        }

        if !dir.pop() {
            bail!("could not find dots.lua")
        }
    }
}
