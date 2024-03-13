//! Tools for spawning programs.

use std::{cell::RefCell, collections::HashMap, os::fd::OwnedFd};

/// Sets an environment variable.
///
/// This does not affect the compositor itself but only programs spawned by the compositor.
pub fn set_env(key: &str, val: &str) {
    get!().set_env(key, val);
}

/// Unsets an environment variable.
///
/// This does not affect the compositor itself but only programs spawned by the compositor.
pub fn unset_env(key: &str) {
    get!().unset_env(key);
}

/// A command to be spawned.
pub struct Command {
    pub(crate) prog: String,
    pub(crate) args: Vec<String>,
    pub(crate) env: HashMap<String, String>,
    pub(crate) fds: RefCell<HashMap<i32, OwnedFd>>,
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
            fds: Default::default(),
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

    /// Sets a file descriptor of the process.
    ///
    /// By default, the process starts with exactly stdin, stdout, and stderr open and all
    /// pointing to `/dev/null`.
    pub fn fd<F: Into<OwnedFd>>(&mut self, idx: i32, fd: F) -> &mut Self {
        self.fds.borrow_mut().insert(idx, fd.into());
        self
    }

    /// Sets the stdin of the process.
    ///
    /// This is equivalent to `fd(0, fd)`.
    pub fn stdin<F: Into<OwnedFd>>(&mut self, fd: F) -> &mut Self {
        self.fd(0, fd)
    }

    /// Sets the stdout of the process.
    ///
    /// This is equivalent to `fd(1, fd)`.
    pub fn stdout<F: Into<OwnedFd>>(&mut self, fd: F) -> &mut Self {
        self.fd(1, fd)
    }

    /// Sets the stderr of the process.
    ///
    /// This is equivalent to `fd(2, fd)`.
    pub fn stderr<F: Into<OwnedFd>>(&mut self, fd: F) -> &mut Self {
        self.fd(2, fd)
    }

    /// Executes the command.
    ///
    /// This consumes all attached file descriptors.
    pub fn spawn(&self) {
        get!().spawn(self);
    }
}
