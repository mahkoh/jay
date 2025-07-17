//! This crate allows you to configure the Jay compositor.
//!
//! A minimal example configuration looks as follows:
//!
//! ```rust
//! use jay_config::{config, quit, reload};
//! use jay_config::input::get_default_seat;
//! use jay_config::keyboard::mods::ALT;
//! use jay_config::keyboard::syms::{SYM_q, SYM_r};
//!
//! fn configure() {
//!     let seat = get_default_seat();
//!     // Create a key binding to exit the compositor.
//!     seat.bind(ALT | SYM_q, || quit());
//!     // Reload the configuration.
//!     seat.bind(ALT | SYM_r, || reload());
//! }
//!
//! config!(configure);
//! ```
//!
//! You should configure your crate to be compiled as a shared library:
//!
//! ```toml
//! [lib]
//! crate-type = ["cdylib"]
//! ```
//!
//! After compiling it, copy the shared library to `$HOME/.config/jay/config.so` and restart
//! the compositor. It should then use your configuration file.
//!
//! Note that you do not have to restart the compositor every time you want to reload your
//! configuration afterwards. Instead, simply invoke the [`reload`] function via a shortcut.

#![allow(
    clippy::zero_prefixed_literal,
    clippy::manual_range_contains,
    clippy::uninlined_format_args,
    clippy::len_zero,
    clippy::single_char_pattern,
    clippy::single_char_add_str,
    clippy::single_match
)]
#![warn(unsafe_op_in_unsafe_fn)]

#[expect(unused_imports)]
use crate::input::Seat;
use {
    crate::{
        _private::ipc::WorkspaceSource, keyboard::ModifiedKeySym, video::Connector, window::Window,
    },
    serde::{Deserialize, Serialize},
    std::{
        fmt::{Debug, Display, Formatter},
        time::Duration,
    },
};

#[macro_use]
mod macros;
#[doc(hidden)]
pub mod _private;
pub mod client;
pub mod embedded;
pub mod exec;
pub mod input;
pub mod io;
pub mod keyboard;
pub mod logging;
pub mod status;
pub mod tasks;
pub mod theme;
pub mod timer;
pub mod video;
pub mod window;
pub mod xwayland;

/// A planar direction.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq)]
pub enum Direction {
    Left,
    Down,
    Up,
    Right,
}

/// A planar axis.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

impl Axis {
    /// Returns the axis orthogonal to `self`.
    pub fn other(self) -> Self {
        match self {
            Self::Horizontal => Self::Vertical,
            Self::Vertical => Self::Horizontal,
        }
    }
}

/// Exits the compositor.
pub fn quit() {
    get!().quit()
}

/// Switches to a different VT.
pub fn switch_to_vt(n: u32) {
    get!().switch_to_vt(n)
}

/// Reloads the configuration.
///
/// If the configuration cannot be reloaded, this function has no effect.
pub fn reload() {
    get!().reload()
}

/// Returns whether this execution of the configuration function is due to a reload.
///
/// This can be used to decide whether the configuration should auto-start programs.
pub fn is_reload() -> bool {
    get!(false).is_reload()
}

/// Sets whether new workspaces are captured by default.
///
/// The default is `true`.
pub fn set_default_workspace_capture(capture: bool) {
    get!().set_default_workspace_capture(capture)
}

/// Returns whether new workspaces are captured by default.
pub fn get_default_workspace_capture() -> bool {
    get!(true).get_default_workspace_capture()
}

/// Toggles whether new workspaces are captured by default.
pub fn toggle_default_workspace_capture() {
    let get = get!();
    get.set_default_workspace_capture(!get.get_default_workspace_capture());
}

/// A workspace.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Workspace(pub u64);

impl Workspace {
    /// Returns whether this workspace existed at the time `Seat::get_workspace` was called.
    pub fn exists(self) -> bool {
        self.0 != 0
    }

    /// Sets whether the workspaces is captured.
    ///
    /// The default is determined by `set_default_workspace_capture`.
    pub fn set_capture(self, capture: bool) {
        get!().set_workspace_capture(self, capture)
    }

    /// Returns whether the workspaces is captured.
    pub fn get_capture(self) -> bool {
        get!(true).get_workspace_capture(self)
    }

    /// Toggles whether the workspaces is captured.
    pub fn toggle_capture(self) {
        let get = get!();
        get.set_workspace_capture(self, !get.get_workspace_capture(self));
    }

    /// Moves this workspace to another output.
    ///
    /// This has no effect if the workspace is not currently being shown.
    pub fn move_to_output(self, output: Connector) {
        get!().move_to_output(WorkspaceSource::Explicit(self), output);
    }

    /// Returns the root container of this workspace.
    ///
    /// If no such container exists, [`Window::exists`] returns false.
    pub fn window(self) -> Window {
        get!(Window(0)).get_workspace_window(self)
    }
}

/// Returns the workspace with the given name.
///
/// Workspaces are identified by their name. Calling this function alone does not create the
/// workspace if it doesn't already exist.
pub fn get_workspace(name: &str) -> Workspace {
    get!(Workspace(0)).get_workspace(name)
}

/// A PCI ID.
///
/// PCI IDs can be used to identify a hardware component. See the Debian [documentation][pci].
///
/// [pci]: https://wiki.debian.org/HowToIdentifyADevice/PCI
#[derive(Serialize, Deserialize, Debug, Copy, Clone, Hash, Eq, PartialEq, Default)]
pub struct PciId {
    pub vendor: u32,
    pub model: u32,
}

impl Display for PciId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04x}:{:04x}", self.vendor, self.model)
    }
}

/// Sets the callback to be called when the display goes idle.
pub fn on_idle<F: FnMut() + 'static>(f: F) {
    get!().on_idle(f)
}

/// Sets the callback to be called when all devices have been enumerated.
///
/// This callback is only invoked once during the lifetime of the compositor. This is a
/// good place to select the DRM device used for rendering.
pub fn on_devices_enumerated<F: FnOnce() + 'static>(f: F) {
    get!().on_devices_enumerated(f)
}

/// Returns the Jay config directory.
pub fn config_dir() -> String {
    get!().config_dir()
}

/// Returns all visible workspaces.
pub fn workspaces() -> Vec<Workspace> {
    get!().workspaces()
}

/// Configures the idle timeout.
///
/// `None` disables the timeout.
///
/// The default is 10 minutes.
pub fn set_idle(timeout: Option<Duration>) {
    get!().set_idle(timeout.unwrap_or_default())
}

/// Configures the idle grace period.
///
/// The grace period starts after the idle timeout expires. During the grace period, the
/// screen goes black but the displays are not yet disabled and the idle callback (set
/// with [`on_idle`]) is not yet called. This is a purely visual effect to inform the user
/// that the machine will soon go idle.
///
/// The default is 5 seconds.
pub fn set_idle_grace_period(timeout: Duration) {
    get!().set_idle_grace_period(timeout)
}

/// Enables or disables explicit sync.
///
/// Calling this after the compositor has started has no effect.
///
/// The default is `true`.
pub fn set_explicit_sync_enabled(enabled: bool) {
    get!().set_explicit_sync_enabled(enabled);
}

/// Enables or disables dragging of tiles and workspaces.
///
/// The default is `true`.
pub fn set_ui_drag_enabled(enabled: bool) {
    get!().set_ui_drag_enabled(enabled);
}

/// Sets the distance at which ui dragging starts.
///
/// The default is `10`.
pub fn set_ui_drag_threshold(threshold: i32) {
    get!().set_ui_drag_threshold(threshold);
}

/// Enables or disables the color-management protocol.
///
/// The default is `false`.
///
/// Affected applications must be restarted for this to take effect.
pub fn set_color_management_enabled(enabled: bool) {
    get!().set_color_management_enabled(enabled);
}

/// Sets whether floating windows are shown above fullscreen windows.
///
/// The default is `false`.
pub fn set_float_above_fullscreen(above: bool) {
    get!().set_float_above_fullscreen(above);
}

/// Gets whether floating windows are shown above fullscreen windows.
pub fn get_float_above_fullscreen() -> bool {
    get!().get_float_above_fullscreen()
}

/// Toggles whether floating windows are shown above fullscreen windows.
///
/// The default is `false`.
pub fn toggle_float_above_fullscreen() {
    set_float_above_fullscreen(!get_float_above_fullscreen())
}

/// Sets whether floating windows always show a pin icon.
///
/// Clicking on the pin icon toggles the pin mode. See [`Seat::toggle_float_pinned`].
///
/// The icon is always shown if the window is pinned. This setting only affects unpinned
/// windows.
pub fn set_show_float_pin_icon(show: bool) {
    get!().set_show_float_pin_icon(show);
}

/// Sets whether the built-in bar is shown.
///
/// The default is `true`.
pub fn set_show_bar(show: bool) {
    get!().set_show_bar(show)
}

/// Returns whether the built-in bar is shown.
pub fn get_show_bar() -> bool {
    get!(true).get_show_bar()
}

/// Toggles whether the built-in bar is shown.
pub fn toggle_show_bar() {
    let get = get!();
    get.set_show_bar(!get.get_show_bar());
}

/// Sets a callback to run when this config is unloaded.
///
/// Only one callback can be set at a time. If another callback is already set, it will be
/// dropped without being run.
///
/// This function can be used to terminate threads and clear reference cycles.
pub fn on_unload(f: impl FnOnce() + 'static) {
    get!().on_unload(f);
}
