use {
    crate::{
        client::ClientError,
        ifs::wl_seat::WlSeat,
        leaks::Tracker,
        object::{Object, Version},
        utils::{errorfmt::ErrorFmt, numcell::NumCell, oserror::OsError},
        wire::{wl_keyboard::*, WlKeyboardId, WlSurfaceId},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub const REPEAT_INFO_SINCE: Version = Version(4);

#[allow(dead_code)]
const NO_KEYMAP: u32 = 0;
pub(super) const XKB_V1: u32 = 1;

pub(super) const RELEASED: u32 = 0;
pub(super) const PRESSED: u32 = 1;

pub struct WlKeyboard {
    id: WlKeyboardId,
    seat: Rc<WlSeat>,
    pub(super) keymap_version: NumCell<u32>,
    pub tracker: Tracker<Self>,
}

impl WlKeyboard {
    pub fn new(id: WlKeyboardId, seat: &Rc<WlSeat>) -> Self {
        Self {
            id,
            seat: seat.clone(),
            keymap_version: NumCell::new(0),
            tracker: Default::default(),
        }
    }

    pub fn send_keymap(&self) {
        let map = self.seat.global.effective_kb_map.get();
        let fd = match self.seat.keymap_fd(&map) {
            Ok(fd) => fd,
            Err(e) => {
                log::error!(
                    "Could not creat a file descriptor to transfer the keymap to client {}: {}",
                    self.seat.client.id,
                    ErrorFmt(e)
                );
                return;
            }
        };
        self.seat.client.event(Keymap {
            self_id: self.id,
            format: XKB_V1,
            fd,
            size: map.map_len as _,
        });
        self.keymap_version
            .set(self.seat.global.keymap_version.get());
    }

    pub fn send_enter(self: &Rc<Self>, serial: u32, surface: WlSurfaceId, keys: &[u32]) {
        if self.keymap_version.get() != self.seat.global.keymap_version.get() {
            self.send_keymap();
        }
        self.seat.client.event(Enter {
            self_id: self.id,
            serial,
            surface,
            keys,
        })
    }

    pub fn send_leave(self: &Rc<Self>, serial: u32, surface: WlSurfaceId) {
        self.seat.client.event(Leave {
            self_id: self.id,
            serial,
            surface,
        })
    }

    pub fn send_key(self: &Rc<Self>, serial: u32, time: u32, key: u32, state: u32) {
        self.seat.client.event(Key {
            self_id: self.id,
            serial,
            time,
            key,
            state,
        })
    }

    pub fn send_modifiers(
        self: &Rc<Self>,
        serial: u32,
        mods_depressed: u32,
        mods_latched: u32,
        mods_locked: u32,
        group: u32,
    ) {
        self.seat.client.event(Modifiers {
            self_id: self.id,
            serial,
            mods_depressed,
            mods_latched,
            mods_locked,
            group,
        })
    }

    pub fn send_repeat_info(self: &Rc<Self>, rate: i32, delay: i32) {
        self.seat.client.event(RepeatInfo {
            self_id: self.id,
            rate,
            delay,
        })
    }
}

impl WlKeyboardRequestHandler for WlKeyboard {
    type Error = WlKeyboardError;

    fn release(&self, _req: Release, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.seat.keyboards.remove(&self.id);
        self.seat.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = WlKeyboard;
    version = self.seat.version;
}

impl Object for WlKeyboard {}

simple_add_obj!(WlKeyboard);

#[derive(Debug, Error)]
pub enum WlKeyboardError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not create a keymap memfd")]
    KeymapMemfd(#[source] OsError),
    #[error("Could not copy the keymap")]
    KeymapCopy(#[source] OsError),
}
efrom!(WlKeyboardError, ClientError);
