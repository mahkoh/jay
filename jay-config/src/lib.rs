use {
    crate::keyboard::{keymap::Keymap, ModifiedKeySym},
    bincode::{Decode, Encode},
    std::{
        collections::HashMap,
        fmt::{Debug, Display, Formatter},
        time::Duration,
    },
};

#[macro_use]
mod macros;
#[doc(hidden)]
pub mod _private;
pub mod drm;
pub mod embedded;
pub mod input;
pub mod keyboard;
pub mod status;
pub mod theme;

#[derive(Encode, Decode, Copy, Clone, Debug)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Encode, Decode, Copy, Clone, Debug, Eq, PartialEq)]
pub enum Direction {
    Unspecified,
    Left,
    Down,
    Up,
    Right,
}

#[derive(Encode, Decode, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

impl Axis {
    pub fn other(self) -> Self {
        match self {
            Self::Horizontal => Self::Vertical,
            Self::Vertical => Self::Horizontal,
        }
    }
}

pub fn quit() {
    get!().quit()
}

pub fn switch_to_vt(n: u32) {
    get!().switch_to_vt(n)
}

pub fn set_env(key: &str, val: &str) {
    get!().set_env(key, val);
}

pub struct Command {
    prog: String,
    args: Vec<String>,
    env: HashMap<String, String>,
}

impl Command {
    pub fn new(prog: &str) -> Self {
        Self {
            prog: prog.to_string(),
            args: vec![],
            env: Default::default(),
        }
    }

    pub fn arg(&mut self, arg: &str) -> &mut Self {
        self.args.push(arg.to_string());
        self
    }

    pub fn env(&mut self, key: &str, val: &str) -> &mut Self {
        self.env.insert(key.to_string(), val.to_string());
        self
    }

    pub fn spawn(&self) {
        get!().spawn(self);
    }
}

#[derive(Encode, Decode, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Workspace(pub u64);

pub fn get_workspace(name: &str) -> Workspace {
    get!(Workspace(0)).get_workspace(name)
}

#[derive(Encode, Decode, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Timer(pub u64);

pub fn get_timer(name: &str) -> Timer {
    get!(Timer(0)).get_timer(name)
}

impl Timer {
    pub fn program(self, initial: Duration, periodic: Option<Duration>) {
        get!().program_timer(self, Some(initial), periodic);
    }

    pub fn cancel(self) {
        get!().program_timer(self, None, None);
    }

    pub fn remove(self) {
        get!().remove_timer(self);
    }

    pub fn on_tick<F: Fn() + 'static>(self, f: F) {
        get!().on_timer_tick(self, f);
    }
}

pub fn reload() {
    get!().reload()
}

pub fn is_reload() -> bool {
    get!(false).is_reload()
}

#[derive(Encode, Decode, Debug, Copy, Clone, Hash, Eq, PartialEq, Default)]
pub struct PciId {
    pub vendor: u32,
    pub model: u32,
}

impl Display for PciId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04x}:{:04x}", self.vendor, self.model)
    }
}
