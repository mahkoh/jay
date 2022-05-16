//! Tools for spawning programs.

use std::collections::HashMap;

/// Sets an environment variable.
///
/// This does not affect the compositor itself but only programs spawned by the compositor.
pub fn set_env(key: &str, val: &str) {
    get!().set_env(key, val);
}

/// A command to be spawned.
pub struct Command {
    pub(crate) prog: String,
    pub(crate) args: Vec<String>,
    pub(crate) env: HashMap<String, String>,
}

impl Command {
    /// Creates a new command to be spawned.
    ///
    /// `prog` should be the path to the program being spawned. If `prog` does not contain
    /// a `/`, then it will be searched in `PATH` similar to how a shell would do it.
    ///
    /// The first argument passed to `prog`, `argv[0]`, is `prog` itself.
    pub fn new(prog: &str) -> Self {
        Self {
            prog: prog.to_string(),
            args: vec![],
            env: Default::default(),
        }
    }

    /// Adds an argument to be passed to the command.
    pub fn arg(&mut self, arg: &str) -> &mut Self {
        self.args.push(arg.to_string());
        self
    }

    /// Sets an environment variable for this command only.
    pub fn env(&mut self, key: &str, val: &str) -> &mut Self {
        self.env.insert(key.to_string(), val.to_string());
        self
    }

    /// Executes the command.
    pub fn spawn(&self) {
        get!().spawn(self);
    }
}
