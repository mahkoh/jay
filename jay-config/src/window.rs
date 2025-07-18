//! Tools for inspecting and manipulating windows.

use {
    crate::{
        Axis, Direction, Workspace,
        client::{Client, ClientCriterion},
        input::Seat,
    },
    serde::{Deserialize, Serialize},
    std::ops::Deref,
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

bitflags! {
    /// The content type of a window.
    #[derive(Serialize, Deserialize, Copy, Clone, Hash, Eq, PartialEq)]
    pub struct ContentType(pub u64) {
        /// No content type.
        pub const NO_CONTENT_TYPE = 1 << 0,
        /// Photo content type.
        pub const PHOTO_CONTENT = 1 << 1,
        /// Video content type.
        pub const VIDEO_CONTENT = 1 << 2,
        /// Game content type.
        pub const GAME_CONTENT = 1 << 3,
    }
}

/// The tile state of a window.
#[non_exhaustive]
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum TileState {
    /// The window is tiled.
    Tiled,
    /// The window is floating.
    Floating,
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

    /// Returns the content type of the window.
    pub fn content_type(self) -> ContentType {
        get!(ContentType(0)).content_type(self)
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

/// A window matcher.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct WindowMatcher(pub u64);

/// A matched window.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct MatchedWindow {
    pub(crate) matcher: WindowMatcher,
    pub(crate) window: Window,
}

/// A criterion for matching a window.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
#[non_exhaustive]
pub enum WindowCriterion<'a> {
    /// Matches if the contained matcher matches.
    Matcher(WindowMatcher),
    /// Matches if the contained criterion does not match.
    Not(&'a WindowCriterion<'a>),
    /// Matches if the window has one of the types.
    Types(WindowType),
    /// Matches if all of the contained criteria match.
    All(&'a [WindowCriterion<'a>]),
    /// Matches if any of the contained criteria match.
    Any(&'a [WindowCriterion<'a>]),
    /// Matches if an exact number of the contained criteria match.
    Exactly(usize, &'a [WindowCriterion<'a>]),
    /// Matches if the window's client matches the client criterion.
    Client(&'a ClientCriterion<'a>),
    /// Matches the title of the window verbatim.
    Title(&'a str),
    /// Matches the title of the window with a regular expression.
    TitleRegex(&'a str),
    /// Matches the app-id of the window verbatim.
    AppId(&'a str),
    /// Matches the app-id of the window with a regular expression.
    AppIdRegex(&'a str),
    /// Matches if the window is floating.
    Floating,
    /// Matches if the window is visible.
    Visible,
    /// Matches if the window has the urgency flag set.
    Urgent,
    /// Matches if the window has the keyboard focus of the seat.
    Focus(Seat),
    /// Matches if the window is fullscreen.
    Fullscreen,
    /// Matches if the window has/hasn't just been mapped.
    ///
    /// This is true for one iteration of the compositor's main loop immediately after the
    /// window has been mapped.
    JustMapped,
    /// Matches the toplevel-tag of the window verbatim.
    Tag(&'a str),
    /// Matches the toplevel-tag of the window with a regular expression.
    TagRegex(&'a str),
    /// Matches the X class of the window verbatim.
    XClass(&'a str),
    /// Matches the X class of the window with a regular expression.
    XClassRegex(&'a str),
    /// Matches the X instance of the window verbatim.
    XInstance(&'a str),
    /// Matches the X instance of the window with a regular expression.
    XInstanceRegex(&'a str),
    /// Matches the X role of the window verbatim.
    XRole(&'a str),
    /// Matches the X role of the window with a regular expression.
    XRoleRegex(&'a str),
    /// Matches the workspace the window.
    Workspace(Workspace),
    /// Matches the workspace name of the window verbatim.
    WorkspaceName(&'a str),
    /// Matches the workspace name of the window with a regular expression.
    WorkspaceNameRegex(&'a str),
    /// Matches if the window has one of the content types.
    ContentTypes(ContentType),
}

impl WindowCriterion<'_> {
    /// Converts the criterion to a matcher.
    pub fn to_matcher(self) -> WindowMatcher {
        get!(WindowMatcher(0)).create_window_matcher(self)
    }

    /// Binds a function to execute when the criterion matches a window.
    ///
    /// This leaks the matcher.
    pub fn bind<F: FnMut(MatchedWindow) + 'static>(self, cb: F) {
        self.to_matcher().bind(cb);
    }

    /// Sets whether newly mapped windows that match this criterion get the keyboard focus.
    ///
    /// If a window matches any criterion for which this is false, the window will not be
    /// automatically focused.
    ///
    /// This leaks the matcher.
    pub fn set_auto_focus(self, auto_focus: bool) {
        self.to_matcher().set_auto_focus(auto_focus);
    }

    /// Sets whether newly mapped windows that match this matcher are mapped tiling or
    /// floating.
    ///
    /// If multiple such window matchers match a window, the used tile state is
    /// unspecified.
    ///
    /// This leaks the matcher.
    pub fn set_initial_tile_state(self, tile_state: TileState) {
        self.to_matcher().set_initial_tile_state(tile_state);
    }
}

impl WindowMatcher {
    /// Destroys the matcher.
    ///
    /// Any bound callback will no longer be executed.
    pub fn destroy(self) {
        get!().destroy_window_matcher(self);
    }

    /// Sets a function to execute when the criterion matches a window.
    ///
    /// Replaces any already bound callback.
    pub fn bind<F: FnMut(MatchedWindow) + 'static>(self, cb: F) {
        get!().set_window_matcher_handler(self, cb);
    }

    /// Sets whether newly mapped windows that match this matcher get the keyboard focus.
    ///
    /// If a window matches any matcher for which this is false, the window will not be
    /// automatically focused.
    pub fn set_auto_focus(self, auto_focus: bool) {
        get!().set_window_matcher_auto_focus(self, auto_focus);
    }

    /// Sets whether newly mapped windows that match this matcher are mapped tiling or
    /// floating.
    ///
    /// If multiple such window matchers match a window, the used tile state is
    /// unspecified.
    pub fn set_initial_tile_state(self, tile_state: TileState) {
        get!().set_window_matcher_initial_tile_state(self, tile_state);
    }
}

impl MatchedWindow {
    /// Returns the window that matched.
    pub fn window(self) -> Window {
        self.window
    }

    /// Returns the matcher.
    pub fn matcher(self) -> WindowMatcher {
        self.matcher
    }

    /// Latches a function to be executed when the window no longer matches the criteria.
    pub fn latch<F: FnOnce() + 'static>(self, cb: F) {
        get!().set_window_matcher_latch_handler(self.matcher, self.window, cb);
    }
}

impl Deref for MatchedWindow {
    type Target = Window;

    fn deref(&self) -> &Self::Target {
        &self.window
    }
}
