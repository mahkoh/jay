mod handler;

#[cfg(feature = "it")]
use crate::it::test_config::TEST_CONFIG_ENTRY;
use {
    crate::{
        backend::{ConnectorId, DrmDeviceId, InputDeviceId},
        client::{Client, ClientCaps},
        config::handler::ConfigProxyHandler,
        ifs::wl_seat::SeatId,
        state::State,
        tree::ToplevelData,
        utils::{
            clonecell::CloneCell,
            nice::{JAY_NO_REALTIME, dont_allow_config_so},
            numcell::NumCell,
            ptr_ext::PtrExt,
            toplevel_identifier::ToplevelIdentifier,
            unlink_on_drop::UnlinkOnDrop,
            xrd::xrd,
        },
    },
    bincode::Options,
    jay_config::{
        _private::{
            ConfigEntry, VERSION, bincode_ops,
            ipc::{InitMessage, ServerFeature, ServerMessage, V1InitMessage},
        },
        input::{InputDevice, Seat, SwitchEvent},
        keyboard::{mods::Modifiers, syms::KeySym},
        video::{Connector, DrmDevice},
        window::{self, TileState},
    },
    libloading::Library,
    std::{cell::Cell, io, mem, path::Path, ptr, rc::Rc},
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Could not load the config library")]
    CouldNotLoadLibrary(#[source] libloading::Error),
    #[error("Config library does not contain the entry symbol")]
    LibraryDoesNotContainEntry(#[source] libloading::Error),
    #[error("Could not determine the config directory")]
    ConfigDirNotSet,
    #[error("Could not copy the config file")]
    CopyConfigFile(#[source] io::Error),
    #[error("XDG_RUNTIME_DIR is not set")]
    XrdNotSet,
    #[error("Custom config.so is not permitted")]
    NotPermitted,
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

    pub fn invoke_shortcut(&self, seat: SeatId, shortcut: &InvokedShortcut) {
        let msg = if shortcut.unmasked_mods == shortcut.effective_mods {
            ServerMessage::InvokeShortcut {
                seat: Seat(seat.raw() as _),
                mods: shortcut.effective_mods,
                sym: shortcut.sym,
            }
        } else {
            ServerMessage::InvokeShortcut2 {
                seat: Seat(seat.raw() as _),
                unmasked_mods: shortcut.unmasked_mods,
                effective_mods: shortcut.effective_mods,
                sym: shortcut.sym,
            }
        };
        self.send(&msg);
    }

    pub fn new_drm_dev(&self, dev: DrmDeviceId) {
        self.send(&ServerMessage::NewDrmDev {
            device: DrmDevice(dev.raw() as _),
        });
    }

    pub fn del_drm_dev(&self, dev: DrmDeviceId) {
        self.send(&ServerMessage::DelDrmDev {
            device: DrmDevice(dev.raw() as _),
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

    pub fn devices_enumerated(&self) {
        self.send(&ServerMessage::DevicesEnumerated);
    }

    pub fn clear(&self) {
        self.send(&ServerMessage::Clear);
    }

    pub fn idle(&self) {
        self.send(&ServerMessage::Idle);
    }

    pub fn switch_event(&self, seat: SeatId, input_device: InputDeviceId, event: SwitchEvent) {
        self.send(&ServerMessage::SwitchEvent {
            seat: Seat(seat.raw() as _),
            input_device: InputDevice(input_device.raw() as _),
            event,
        });
    }

    pub fn toplevel_removed(&self, id: ToplevelIdentifier) {
        let Some(handler) = self.handler.get() else {
            return;
        };
        if let Some(win) = handler.windows_from_tl_id.remove(&id) {
            handler.windows_to_tl_id.remove(&win);
        }
    }

    pub fn auto_focus(&self, data: &ToplevelData) -> bool {
        let Some(handler) = self.handler.get() else {
            return true;
        };
        handler.auto_focus(data)
    }

    pub fn initial_tile_state(&self, data: &ToplevelData) -> Option<TileState> {
        self.handler.get()?.initial_tile_state(data)
    }

    pub fn update_capabilities(
        &self,
        data: &Rc<Client>,
        bounding_caps: ClientCaps,
        set_bounding_caps: bool,
    ) {
        if let Some(handler) = self.handler.get() {
            handler.update_capabilities(data, bounding_caps, set_bounding_caps);
        }
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
        jay_toml_config::configure();
    }
    unsafe {
        jay_config::_private::client::init(srv_data, srv_unref, srv_handler, msg, size, configure)
    }
}

impl ConfigProxy {
    fn new(
        lib: Option<Library>,
        entry: &ConfigEntry,
        state: &Rc<State>,
        path: Option<String>,
    ) -> Self {
        let version = entry.version.min(VERSION);
        let data = Rc::new(ConfigProxyHandler {
            path,
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
            pollable_id: Default::default(),
            pollables: Default::default(),
            window_ids: NumCell::new(1),
            windows_from_tl_id: Default::default(),
            windows_to_tl_id: Default::default(),
            client_matcher_ids: NumCell::new(1),
            client_matchers: Default::default(),
            client_matcher_cache: Default::default(),
            client_matcher_leafs: Default::default(),
            client_matcher_capabilities: Default::default(),
            client_matcher_bounding_capabilities: Default::default(),
            window_matcher_ids: NumCell::new(1),
            window_matchers: Default::default(),
            window_matcher_cache: Default::default(),
            window_matcher_leafs: Default::default(),
            window_matcher_std_kinds: state.tl_matcher_manager.kind(window::CLIENT_WINDOW),
            window_matcher_no_auto_focus: Default::default(),
            window_matcher_initial_tile_state: Default::default(),
        });
        let init_msg = bincode_ops()
            .serialize(&InitMessage::V1(V1InitMessage {}))
            .unwrap();
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
        self.send(&ServerMessage::Features {
            features: vec![ServerFeature::MOD_MASK, ServerFeature::SHOW_WORKSPACE_ON],
        });
        self.send(&ServerMessage::Configure { reload });
    }

    pub fn default(state: &Rc<State>) -> Self {
        let entry = ConfigEntry {
            version: VERSION,
            init: default_client_init,
            unref: jay_config::_private::client::unref,
            handle_msg: jay_config::_private::client::handle_msg,
        };
        Self::new(None, &entry, state, None)
    }

    #[cfg(feature = "it")]
    pub fn for_test(state: &Rc<State>) -> Self {
        Self::new(None, &TEST_CONFIG_ENTRY, state, None)
    }

    pub fn from_config_dir(state: &Rc<State>) -> Result<Self, ConfigError> {
        if dont_allow_config_so() {
            if have_config_so(state.config_dir.as_deref()) {
                log::warn!("Not loading config.so because");
                log::warn!("  1. Jay was started with CAP_SYS_NICE");
                log::warn!("  2. Jay was not started with {}=1", JAY_NO_REALTIME);
                log::warn!("  3. The scheduler was elevated to SCHED_RR");
                log::warn!(
                    "  4. Jay was not compiled with {}=1",
                    jay_allow_realtime_config_so!(),
                );
            }
            return Err(ConfigError::NotPermitted);
        }
        let dir = match state.config_dir.as_deref() {
            Some(d) => d,
            _ => return Err(ConfigError::ConfigDirNotSet),
        };
        let file = format!("{}/{CONFIG_SO}", dir);
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
        // temporary copy with an incrementing number and load the library
        // from there.
        let xrd = match xrd() {
            Some(x) => x,
            _ => return Err(ConfigError::XrdNotSet),
        };
        let copy = format!(
            "{}/.jay_config.so.{}.{}",
            xrd,
            uapi::getpid(),
            state.config_file_id.fetch_add(1)
        );
        let _ = uapi::unlink(copy.as_str());
        if let Err(e) = std::fs::copy(path, &copy) {
            return Err(ConfigError::CopyConfigFile(e));
        }
        let unlink = UnlinkOnDrop(&copy);
        let lib = match unsafe { Library::new(&copy) } {
            Ok(l) => l,
            Err(e) => return Err(ConfigError::CouldNotLoadLibrary(e)),
        };
        let entry = unsafe { lib.get::<&'static ConfigEntry>(b"JAY_CONFIG_ENTRY_V1\0") };
        let entry = match entry {
            Ok(e) => *e,
            Err(e) => return Err(ConfigError::LibraryDoesNotContainEntry(e)),
        };
        mem::forget(unlink);
        Ok(Self::new(Some(lib), entry, state, Some(copy)))
    }
}

unsafe extern "C" fn unref(data: *const u8) {
    let server = data as *const ConfigProxyHandler;
    unsafe {
        drop(Rc::from_raw(server));
    }
}

unsafe extern "C" fn handle_msg(data: *const u8, msg: *const u8, size: usize) {
    unsafe {
        let server = (data as *const ConfigProxyHandler).deref();
        if server.dropped.get() {
            return;
        }
        let rc = Rc::from_raw(server);
        let msg = std::slice::from_raw_parts(msg, size);
        rc.handle_request(msg);
        mem::forget(rc);
    }
}

pub struct InvokedShortcut {
    pub unmasked_mods: Modifiers,
    pub effective_mods: Modifiers,
    pub sym: KeySym,
}

const CONFIG_SO: &str = "config.so";

pub fn have_config_so(config_dir: Option<&str>) -> bool {
    let Some(dir) = config_dir else {
        return false;
    };
    let mut dir = dir.to_owned();
    dir.push_str("/");
    dir.push_str(CONFIG_SO);
    Path::new(&dir).exists()
}
