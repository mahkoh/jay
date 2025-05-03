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
