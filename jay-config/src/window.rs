//! Tools for inspecting and manipulating windows.

use {
    crate::{Axis, Direction, Workspace, client::Client},
    serde::{Deserialize, Serialize},
};

/// A toplevel window.
///
/// A toplevel window is anything that can be stored within a container tile or within a
/// floating window.
///
/// There are currently four types of windows:
///
/// - Containers
/// - Placeholders that take the place of a window when it goes fullscreen
/// - XDG toplevels
/// - X windows
///
/// You can find out the type of a window by using the [`Window::type_`] function.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Window(pub u64);

bitflags! {
    /// The type of a window.
    #[derive(Serialize, Deserialize, Copy, Clone, Hash, Eq, PartialEq)]
    pub struct WindowType(pub u64) {
        /// A container.
        pub const CONTAINER = 1 << 0,
        /// A placeholder.
        pub const PLACEHOLDER = 1 << 1,
        /// An XDG toplevel.
        pub const XDG_TOPLEVEL = 1 << 2,
        /// An X window.
        pub const X_WINDOW = 1 << 3,
    }
}

/// A window created by a client.
///
/// This is the same as `XDG_TOPLEVEL | X_WINDOW`.
pub const CLIENT_WINDOW: WindowType = WindowType(XDG_TOPLEVEL.0 | X_WINDOW.0);

impl Window {
    /// Returns whether the window exists.
    pub fn exists(self) -> bool {
        self.0 != 0 && get!(false).window_exists(self)
    }

    /// Returns whether the window does not exist.
    ///
    /// This is a shorthand for `!self.exists()`.
    pub fn does_not_exist(self) -> bool {
        !self.exists()
    }

    /// Returns the client of the window.
    ///
    /// If the window does not have a client, [`Client::exists`] return false.
    pub fn client(self) -> Client {
        get!(Client(0)).window_client(self)
    }

    /// Returns the title of the window.
    pub fn title(self) -> String {
        get!().window_title(self)
    }

    /// Returns the type of the window.
    pub fn type_(self) -> WindowType {
        get!(WindowType(0)).window_type(self)
    }

    /// Returns the identifier of the window.
    ///
    /// This is the identifier used in the `ext-foreign-toplevel-list-v1` protocol.
    pub fn id(self) -> String {
        get!().window_id(self)
    }

    /// Returns whether this window is visible.
    pub fn is_visible(self) -> bool {
        get!().window_is_visible(self)
    }

    /// Returns the parent of this window.
    ///
    /// If this window has no parent, [`Window::exists`] returns false.
    pub fn parent(self) -> Window {
        get!(Window(0)).window_parent(self)
    }

    /// Returns the children of this window.
    ///
    /// Only containers have children.
    pub fn children(self) -> Vec<Window> {
        get!().window_children(self)
    }

    /// Moves the window in the specified direction.
    pub fn move_(self, direction: Direction) {
        get!().window_move(self, direction)
    }

    /// Returns whether the parent-container of the window is in mono-mode.
    pub fn mono(self) -> bool {
        get!(false).window_mono(self)
    }

    /// Sets whether the parent-container of the window is in mono-mode.
    pub fn set_mono(self, mono: bool) {
        get!().set_window_mono(self, mono)
    }

    /// Toggles whether the parent-container of the window is in mono-mode.
    pub fn toggle_mono(self) {
        self.set_mono(!self.mono());
    }

    /// Returns the split axis of the parent-container of the window.
    pub fn split(self) -> Axis {
        get!(Axis::Horizontal).window_split(self)
    }

    /// Sets the split axis of the parent-container of the window.
    pub fn set_split(self, axis: Axis) {
        get!().set_window_split(self, axis)
    }

    /// Toggles the split axis of the parent-container of the window.
    pub fn toggle_split(self) {
        self.set_split(self.split().other());
    }

    /// Creates a new container with the specified split in place of the window.
    pub fn create_split(self, axis: Axis) {
        get!().create_window_split(self, axis);
    }

    /// Requests the window to be closed.
    pub fn close(self) {
        get!().close_window(self);
    }

    /// Returns whether the window is floating.
    pub fn floating(self) -> bool {
        get!().get_window_floating(self)
    }
    /// Sets whether the window is floating.
    pub fn set_floating(self, floating: bool) {
        get!().set_window_floating(self, floating);
    }

    /// Toggles whether the window is floating.
    ///
    /// You can do the same by double-clicking on the header.
    pub fn toggle_floating(self) {
        self.set_floating(!self.floating());
    }

    /// Returns the workspace that this window belongs to.
    ///
    /// If no such workspace exists, `exists` returns `false` for the returned workspace.
    pub fn workspace(self) -> Workspace {
        get!(Workspace(0)).get_window_workspace(self)
    }

    /// Moves the window to the workspace.
    pub fn set_workspace(self, workspace: Workspace) {
        get!().set_window_workspace(self, workspace)
    }

    /// Toggles whether the currently focused window is fullscreen.
    pub fn toggle_fullscreen(self) {
        self.set_fullscreen(!self.fullscreen())
    }
    /// Returns whether the window is fullscreen.
    pub fn fullscreen(self) -> bool {
        get!(false).get_window_fullscreen(self)
    }

    /// Sets whether the window is fullscreen.
    pub fn set_fullscreen(self, fullscreen: bool) {
        get!().set_window_fullscreen(self, fullscreen)
    }

    /// Gets whether the window is pinned.
    ///
    /// If a floating window is pinned, it will stay visible even when switching to a
    /// different workspace.
    pub fn float_pinned(self) -> bool {
        get!().get_window_pinned(self)
    }

    /// Sets whether the window is pinned.
    pub fn set_float_pinned(self, pinned: bool) {
        get!().set_window_pinned(self, pinned);
    }

    /// Toggles whether the window is pinned.
    pub fn toggle_float_pinned(self) {
        self.set_float_pinned(!self.float_pinned());
    }
}
