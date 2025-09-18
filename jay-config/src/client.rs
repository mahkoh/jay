//! Tools for inspecting and manipulating clients.

use {
    serde::{Deserialize, Serialize},
    std::ops::Deref,
};

/// A client connected to the compositor.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Client(pub u64);

impl Client {
    /// Returns whether the client exists.
    pub fn exists(self) -> bool {
        self.0 != 0 && get!(false).client_exists(self)
    }

    /// Returns whether the client does not exist.
    ///
    /// This is a shorthand for `!self.exists()`.
    pub fn does_not_exist(self) -> bool {
        !self.exists()
    }

    /// Returns whether this client is XWayland.
    pub fn is_xwayland(self) -> bool {
        get!(false).client_is_xwayland(self)
    }

    /// Disconnects the client.
    pub fn kill(self) {
        get!().client_kill(self)
    }
}

/// Returns all current clients.
pub fn clients() -> Vec<Client> {
    get!().clients()
}

/// A client matcher.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct ClientMatcher(pub u64);

/// A matched client.
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct MatchedClient {
    pub(crate) matcher: ClientMatcher,
    pub(crate) client: Client,
}

/// A criterion for matching a client.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
#[non_exhaustive]
pub enum ClientCriterion<'a> {
    /// Matches if the contained matcher matches.
    Matcher(ClientMatcher),
    /// Matches if the contained criterion does not match.
    Not(&'a ClientCriterion<'a>),
    /// Matches if all of the contained criteria match.
    All(&'a [ClientCriterion<'a>]),
    /// Matches if any of the contained criteria match.
    Any(&'a [ClientCriterion<'a>]),
    /// Matches if an exact number of the contained criteria match.
    Exactly(usize, &'a [ClientCriterion<'a>]),
    /// Matches the engine name of the client's sandbox verbatim.
    SandboxEngine(&'a str),
    /// Matches the engine name of the client's sandbox with a regular expression.
    SandboxEngineRegex(&'a str),
    /// Matches the app id of the client's sandbox verbatim.
    SandboxAppId(&'a str),
    /// Matches the app id of the client's sandbox with a regular expression.
    SandboxAppIdRegex(&'a str),
    /// Matches the instance id of the client's sandbox verbatim.
    SandboxInstanceId(&'a str),
    /// Matches the instance id of the client's sandbox with a regular expression.
    SandboxInstanceIdRegex(&'a str),
    /// Matches if the client is sandboxed.
    Sandboxed,
    /// Matches the user ID of the client.
    Uid(i32),
    /// Matches the process ID of the client.
    Pid(i32),
    /// Matches if the client is Xwayland.
    IsXwayland,
    /// Matches the `/proc/pid/comm` of the client verbatim.
    Comm(&'a str),
    /// Matches the `/proc/pid/comm` of the client with a regular expression.
    CommRegex(&'a str),
    /// Matches the `/proc/pid/exe` of the client verbatim.
    Exe(&'a str),
    /// Matches the `/proc/pid/exe` of the client with a regular expression.
    ExeRegex(&'a str),
}

impl ClientCriterion<'_> {
    /// Converts the criterion to a matcher.
    pub fn to_matcher(self) -> ClientMatcher {
        get!(ClientMatcher(0)).create_client_matcher(self)
    }

    /// Binds a function to execute when the criterion matches a client.
    ///
    /// This leaks the matcher.
    pub fn bind<F: FnMut(MatchedClient) + 'static>(self, cb: F) {
        self.to_matcher().bind(cb);
    }

    /// Sets the capabilities granted to clients matching this matcher.
    ///
    /// This leaks the matcher.
    pub fn set_capabilities(self, caps: ClientCapabilities) {
        self.to_matcher().set_capabilities(caps);
    }

    /// Sets the upper capability bounds for clients in sandboxes created by this client.
    ///
    /// This leaks the matcher.
    pub fn set_sandbox_bounding_capabilities(self, caps: ClientCapabilities) {
        self.to_matcher().set_sandbox_bounding_capabilities(caps);
    }
}

impl ClientMatcher {
    /// Destroys the matcher.
    ///
    /// Any bound callback will no longer be executed.
    pub fn destroy(self) {
        get!().destroy_client_matcher(self);
    }

    /// Sets a function to execute when the criterion matches a client.
    ///
    /// Replaces any already bound callback.
    pub fn bind<F: FnMut(MatchedClient) + 'static>(self, cb: F) {
        get!().set_client_matcher_handler(self, cb);
    }

    /// Sets the capabilities granted to clients matching this matcher.
    ///
    /// If multiple matchers match a client, the capabilities are added.
    ///
    /// If no matcher matches a client, it is granted the default capabilities depending
    /// on whether it's sandboxed or not. If it is not sandboxed, it is granted the
    /// capabilities [`CC_LAYER_SHELL`] and [`CC_DRM_LEASE`]. Otherwise it is granted the
    /// capability [`CC_DRM_LEASE`].
    ///
    /// Regardless of any capabilities set through this function, the capabilities of the
    /// client can never exceed its bounding capabilities.
    pub fn set_capabilities(self, caps: ClientCapabilities) {
        get!().set_client_matcher_capabilities(self, caps);
    }

    /// Sets the upper capability bounds for clients in sandboxes created by this client.
    ///
    /// If multiple matchers match a client, the capabilities are added.
    ///
    /// If no matcher matches a client, the bounding capabilities for sandboxes depend on
    /// whether the client is itself sandboxed. If it is sandboxed, the bounding
    /// capabilities are the effective capabilities of the client. Otherwise the bounding
    /// capabilities are all capabilities.
    ///
    /// Regardless of any capabilities set through this function, the capabilities set
    /// through this function can never exceed the client's bounding capabilities.
    pub fn set_sandbox_bounding_capabilities(self, caps: ClientCapabilities) {
        get!().set_client_matcher_bounding_capabilities(self, caps);
    }
}

impl MatchedClient {
    /// Returns the client that matched.
    pub fn client(self) -> Client {
        self.client
    }

    /// Returns the matcher.
    pub fn matcher(self) -> ClientMatcher {
        self.matcher
    }

    /// Latches a function to be executed when the client no longer matches the criteria.
    pub fn latch<F: FnOnce() + 'static>(self, cb: F) {
        get!().set_client_matcher_latch_handler(self.matcher, self.client, cb);
    }
}

impl Deref for MatchedClient {
    type Target = Client;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

bitflags! {
    /// Capabilities granted to a client.
    #[derive(Serialize, Deserialize, Copy, Clone, Hash, Eq, PartialEq)]
    pub struct ClientCapabilities(pub u64) {
        /// Grants access to the `ext_data_control_manager_v1` and
        /// `zwlr_data_control_manager_v1` globals.
        pub const CC_DATA_CONTROL             = 1 << 0,
        /// Grants access to the `zwp_virtual_keyboard_manager_v1` global.
        pub const CC_VIRTUAL_KEYBOARD         = 1 << 1,
        /// Grants access to the `ext_foreign_toplevel_list_v1` global.
        pub const CC_FOREIGN_TOPLEVEL_LIST    = 1 << 2,
        /// Grants access to the `ext_idle_notifier_v1` global.
        pub const CC_IDLE_NOTIFIER            = 1 << 3,
        /// Grants access to the `ext_session_lock_manager_v1` global.
        pub const CC_SESSION_LOCK             = 1 << 4,
        /// Grants access to the `zwlr_layer_shell_v1` global.
        pub const CC_LAYER_SHELL              = 1 << 6,
        /// Grants access to the `ext_image_copy_capture_manager_v1` and
        /// `zwlr_screencopy_manager_v1` globals.
        pub const CC_SCREENCOPY               = 1 << 7,
        /// Grants access to the `ext_transient_seat_manager_v1` global.
        pub const CC_SEAT_MANAGER             = 1 << 8,
        /// Grants access to the `wp_drm_lease_device_v1` global.
        pub const CC_DRM_LEASE                = 1 << 9,
        /// Grants access to the `zwp_input_method_manager_v2` global.
        pub const CC_INPUT_METHOD             = 1 << 10,
        /// Grants access to the `ext_workspace_manager_v1` global.
        pub const CC_WORKSPACE_MANAGER        = 1 << 11,
        /// Grants access to the `zwlr_foreign_toplevel_manager_v1` global.
        pub const CC_FOREIGN_TOPLEVEL_MANAGER = 1 << 12,
        /// Grants access to the `jay_head_manager_v1` and `zwlr_output_manager_v1`
        /// globals.
        pub const CC_HEAD_MANAGER             = 1 << 13,
    }
}
