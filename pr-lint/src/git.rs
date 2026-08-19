use crate::jay_config;
use crate::jay_config::Types;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use std::process::Command;
use std::sync::OnceLock;

pub struct Tree {
    label: &'static str,
    rev: String,
    types: OnceLock<Result<Types, String>>,
}

impl Tree {
    pub fn new(label: &'static str, rev: &str) -> Result<Self> {
        let commit = git(&["rev-parse", "--verify", &format!("{rev}^{{commit}}")])
            .with_context(|| format!("could not resolve revision `{rev}`"))?;
        Ok(Self {
            label,
            rev: String::from_utf8(commit)
                .context("git printed a non-utf8 commit id")?
                .trim()
                .to_string(),
            types: OnceLock::new(),
        })
    }

    pub fn label(&self) -> &'static str {
        self.label
    }

    pub fn types(&self) -> Result<&Types> {
        let types = self
            .types
            .get_or_init(|| jay_config::collect(self).map_err(|e| format!("{e:#}")));
        types.as_ref().map_err(|e| anyhow!("{e}"))
    }

    pub fn files(&self, dir: &str) -> Result<Vec<String>> {
        let out = git(&["ls-tree", "-r", "-z", "--name-only", &self.rev, "--", dir])
            .with_context(|| format!("could not list the files below `{dir}`"))?;
        let mut files = vec![];
        for file in out.split(|b| *b == 0) {
            if file.is_empty() {
                continue;
            }
            let file = String::from_utf8(file.to_vec())
                .context("the tree contains a non-utf8 file name")?;
            files.push(file);
        }
        files.sort();
        Ok(files)
    }

    pub fn read(&self, path: &str) -> Result<String> {
        let out = git(&["cat-file", "blob", &format!("{}:{}", self.rev, path)])
            .with_context(|| format!("could not read `{path}`"))?;
        String::from_utf8(out).with_context(|| format!("`{path}` is not valid utf8"))
    }
}

fn git(args: &[&str]) -> Result<Vec<u8>> {
    let out = Command::new("git")
        .args(args)
        .output()
        .context("could not execute git")?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        bail!("`git {}` failed: {}", args.join(" "), stderr.trim());
    }
    Ok(out.stdout)
}
