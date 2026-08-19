use crate::git::Tree;
use anyhow::Result;
use rayon::prelude::*;
use std::io::Write;
use std::process::exit;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;

mod git;
mod jay_config;
mod rules;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let (Some(base), Some(head), None) = (args.next(), args.next(), args.next()) else {
        eprintln!("usage: pr-lint <base-revision> <head-revision>");
        exit(1);
    };
    let base = Tree::new("the base branch", &base)?;
    let head = Tree::new("this pull request", &head)?;
    let violated = AtomicBool::new(false);
    rules::rules().par_iter().try_for_each(|rule| {
        let violations = rule.check(&base, &head)?;
        let mut stdout = std::io::stdout().lock();
        if violations.is_empty() {
            return Ok(());
        }
        violated.store(true, Relaxed);
        for violation in violations {
            writeln!(stdout)?;
            writeln!(stdout, "{violation}")?;
        }
        writeln!(stdout)?;
        writeln!(stdout, "HINT: {}", rule.hint())?;
        writeln!(stdout)?;
        anyhow::Ok(())
    })?;
    if violated.load(Relaxed) {
        exit(1);
    }
    Ok(())
}
