//! Tools for inspecting and manipulating clients.

use serde::{Deserialize, Serialize};

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
