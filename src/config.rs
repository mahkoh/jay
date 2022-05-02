mod handler;

use {
    crate::{
        backend::{ConnectorId, InputDeviceId},
        config::handler::ConfigProxyHandler,
        ifs::wl_seat::SeatId,
        state::State,
        utils::{
            clonecell::CloneCell, numcell::NumCell, oserror::OsError, ptr_ext::PtrExt,
            unlink_on_drop::UnlinkOnDrop, xrd::xrd,
        },
    },
    jay_config::{
        _private::{
            bincode_ops,
            ipc::{InitMessage, ServerMessage, V1InitMessage},
            ConfigEntry, VERSION,
        },
        drm::Connector,
        input::{InputDevice, Seat},
        keyboard::ModifiedKeySym,
    },
    libloading::Library,
    std::{cell::Cell, mem, ptr, rc::Rc},
    thiserror::Error,
};
#[cfg(feature = "it")]
use crate::it::test_config::TEST_CONFIG_ENTRY;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Could not load the config library")]
    CouldNotLoadLibrary(#[source] libloading::Error),
    #[error("Config library does not contain the entry symbol")]
    LibraryDoesNotContainEntry(#[source] libloading::Error),
    #[error("Could not determine the config directory")]
    ConfigDirNotSet,
    #[error("Could not link the config file")]
    LinkConfigFile(#[source] OsError),
    #[error("XDG_RUNTIME_DIR is not set")]
    XrdNotSet,
}

pub struct ConfigProxy {
    handler: CloneCell<Option<Rc<ConfigProxyHandler>>>,
}

impl ConfigProxy {
    fn send(&self, msg: &ServerMessage) {
        if let Some(handler) = self.handler.get() {
            handler.send(msg);
        }
    }

    pub fn destroy(&self) {
        if let Some(handler) = self.handler.take() {
            unsafe {
                handler.do_drop();
                (handler.unref)(handler.client_data.get());
            }
        }
    }

    pub fn invoke_shortcut(&self, seat: SeatId, modsym: &ModifiedKeySym) {
        self.send(&ServerMessage::InvokeShortcut {
            seat: Seat(seat.raw() as _),
            mods: modsym.mods,
            sym: modsym.sym,
        });
    }

    pub fn new_connector(&self, connector: ConnectorId) {
        self.send(&ServerMessage::NewConnector {
            device: Connector(connector.raw() as _),
        });
    }

    pub fn del_connector(&self, connector: ConnectorId) {
        self.send(&ServerMessage::DelConnector {
            device: Connector(connector.raw() as _),
        });
    }

    pub fn connector_connected(&self, connector: ConnectorId) {
        self.send(&ServerMessage::ConnectorConnect {
            device: Connector(connector.raw() as _),
        });
    }

    pub fn connector_disconnected(&self, connector: ConnectorId) {
        self.send(&ServerMessage::ConnectorDisconnect {
            device: Connector(connector.raw() as _),
        });
    }

    pub fn new_input_device(&self, dev: InputDeviceId) {
        self.send(&ServerMessage::NewInputDevice {
            device: InputDevice(dev.raw() as _),
        });
    }

    pub fn del_input_device(&self, dev: InputDeviceId) {
        self.send(&ServerMessage::DelInputDevice {
            device: InputDevice(dev.raw() as _),
        });
    }

    pub fn graphics_initialized(&self) {
        self.send(&ServerMessage::GraphicsInitialized);
    }

    pub fn clear(&self) {
        self.send(&ServerMessage::Clear);
    }
}

impl Drop for ConfigProxy {
    fn drop(&mut self) {
        self.destroy();
    }
}

unsafe extern "C" fn default_client_init(
    srv_data: *const u8,
    srv_unref: unsafe extern "C" fn(data: *const u8),
    srv_handler: unsafe extern "C" fn(data: *const u8, msg: *const u8, size: usize),
    msg: *const u8,
    size: usize,
) -> *const u8 {
    extern "C" fn configure() {
        default_config::configure();
    }
    jay_config::_private::client::init(srv_data, srv_unref, srv_handler, msg, size, configure)
}

impl ConfigProxy {
    fn new(lib: Option<Library>, entry: &ConfigEntry, state: &Rc<State>) -> Self {
        let version = entry.version.min(VERSION);
        let data = Rc::new(ConfigProxyHandler {
            client_data: Cell::new(ptr::null()),
            dropped: Cell::new(false),
            _lib: lib,
            _version: version,
            unref: entry.unref,
            handle_msg: entry.handle_msg,
            state: state.clone(),
            next_id: NumCell::new(1),
            keymaps: Default::default(),
            bufs: Default::default(),
            workspace_ids: NumCell::new(1),
            workspaces_by_name: Default::default(),
            workspaces_by_id: Default::default(),
            timer_ids: NumCell::new(1),
            timers_by_name: Default::default(),
            timers_by_id: Default::default(),
        });
        let init_msg =
            bincode::encode_to_vec(&InitMessage::V1(V1InitMessage {}), bincode_ops()).unwrap();
        unsafe {
            let client_data = (entry.init)(
                Rc::into_raw(data.clone()) as _,
                unref,
                handle_msg,
                init_msg.as_ptr(),
                init_msg.len(),
            );
            data.client_data.set(client_data);
        }
        Self {
            handler: CloneCell::new(Some(data)),
        }
    }

    pub fn configure(&self, reload: bool) {
        self.send(&ServerMessage::Configure { reload });
    }

    pub fn default(state: &Rc<State>) -> Self {
        let entry = ConfigEntry {
            version: VERSION,
            init: default_client_init,
            unref: jay_config::_private::client::unref,
            handle_msg: jay_config::_private::client::handle_msg,
        };
        Self::new(None, &entry, state)
    }

    #[cfg(feature = "it")]
    pub fn for_test(state: &Rc<State>) -> Self {
        Self::new(None, &TEST_CONFIG_ENTRY, state)
    }

    pub fn from_config_dir(state: &Rc<State>) -> Result<Self, ConfigError> {
        let dir = match state.config_dir.as_deref() {
            Some(d) => d,
            _ => return Err(ConfigError::ConfigDirNotSet),
        };
        let file = format!("{}/config.so", dir);
        unsafe { Self::from_file(&file, state) }
    }

    pub unsafe fn from_file(path: &str, state: &Rc<State>) -> Result<Self, ConfigError> {
        // Here we have to do a bit of a dance to support reloading. glibc will
        // never load a library twice unless it has been unloaded in between.
        // glibc identifies libraries by their file path and by their inode
        // number. If either of those match, glibc considers the libraries
        // identical.  If the inode has not changed then this is not a problem
        // for us since we don't want glibc to do any unnecessary work.
        // However, if the user has created a new config with a new inode, then
        // glibc will still not reload the library if we try to load it from
        // the canonical location ~/.config/jay/config.so since it already has
        // a library with that path loaded. To work around this, create a
        // temporary symlink with an incrementing number and load the library
        // from there.
        let xrd = match xrd() {
            Some(x) => x,
            _ => return Err(ConfigError::XrdNotSet),
        };
        let link = format!(
            "{}/.jay_config.so.{}.{}",
            xrd,
            uapi::getpid(),
            state.config_file_id.fetch_add(1)
        );
        let _ = uapi::unlink(link.as_str());
        if let Err(e) = uapi::symlink(path, link.as_str()) {
            return Err(ConfigError::LinkConfigFile(e.into()));
        }
        let _unlink = UnlinkOnDrop(&link);
        let lib = match Library::new(&link) {
            Ok(l) => l,
            Err(e) => return Err(ConfigError::CouldNotLoadLibrary(e)),
        };
        let entry = lib.get::<&'static ConfigEntry>(b"JAY_CONFIG_ENTRY_V1\0");
        let entry = match entry {
            Ok(e) => *e,
            Err(e) => return Err(ConfigError::LibraryDoesNotContainEntry(e)),
        };
        Ok(Self::new(Some(lib), entry, state))
    }
}

unsafe extern "C" fn unref(data: *const u8) {
    let server = data as *const ConfigProxyHandler;
    drop(Rc::from_raw(server));
}

unsafe extern "C" fn handle_msg(data: *const u8, msg: *const u8, size: usize) {
    let server = (data as *const ConfigProxyHandler).deref();
    if server.dropped.get() {
        return;
    }
    let rc = Rc::from_raw(server);
    let msg = std::slice::from_raw_parts(msg, size);
    rc.handle_request(msg);
    mem::forget(rc);
}
