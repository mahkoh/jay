use {
    crate::{
        client::ClientError,
        ifs::wl_seat::WlSeat,
        leaks::Tracker,
        object::{Object, Version},
        utils::errorfmt::ErrorFmt,
        wire::{wl_keyboard::*, WlKeyboardId, WlSurfaceId},
        xkbcommon::{KeyboardState, KeyboardStateId, ModifierState, XkbCommonError},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub const REPEAT_INFO_SINCE: Version = Version(4);

#[expect(dead_code)]
const NO_KEYMAP: u32 = 0;
pub const XKB_V1: u32 = 1;

pub const RELEASED: u32 = 0;
pub const PRESSED: u32 = 1;

pub struct WlKeyboard {
    id: WlKeyboardId,
    seat: Rc<WlSeat>,
    kb_state_id: Cell<KeyboardStateId>,
    pub tracker: Tracker<Self>,
}

impl WlKeyboard {
    pub fn new(id: WlKeyboardId, seat: &Rc<WlSeat>) -> Self {
        Self {
            id,
            seat: seat.clone(),
            kb_state_id: Cell::new(KeyboardStateId::from_raw(0)),
            tracker: Default::default(),
        }
    }

    pub fn kb_state_id(&self) -> KeyboardStateId {
        self.kb_state_id.get()
    }

    fn send_kb_state(
        &self,
        serial: u64,
        kb_state: &KeyboardState,
        surface_id: WlSurfaceId,
        send_leave: bool,
    ) {
        self.kb_state_id.set(kb_state.id);
        if send_leave {
            self.send_leave(serial, surface_id);
        }
        self.send_keymap(kb_state);
        self.send_enter(serial, surface_id, &kb_state.pressed_keys);
        self.send_modifiers(serial, &kb_state.mods);
    }

    fn send_keymap(&self, state: &KeyboardState) {
        let fd = match self.seat.keymap_fd(state) {
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
            size: state.map_len as _,
        });
    }

    pub fn enter(&self, serial: u64, surface: WlSurfaceId, kb_state: &KeyboardState) {
        if kb_state.id != self.kb_state_id.get() {
            self.send_kb_state(serial, kb_state, surface, false);
        } else {
            self.send_enter(serial, surface, &kb_state.pressed_keys);
            self.send_modifiers(serial, &kb_state.mods);
        }
    }

    fn send_enter(&self, serial: u64, surface: WlSurfaceId, keys: &[u32]) {
        self.seat.client.event(Enter {
            self_id: self.id,
            serial: serial as _,
            surface,
            keys,
        })
    }

    pub fn send_leave(&self, serial: u64, surface: WlSurfaceId) {
        self.seat.client.event(Leave {
            self_id: self.id,
            serial: serial as _,
            surface,
        })
    }

    pub fn on_key(
        &self,
        serial: u64,
        time: u32,
        key: u32,
        state: u32,
        surface: WlSurfaceId,
        kb_state: &KeyboardState,
    ) {
        if self.kb_state_id.get() != kb_state.id {
            self.send_kb_state(serial, kb_state, surface, true);
        }
        self.send_key(serial, time, key, state);
    }

    fn send_key(&self, serial: u64, time: u32, key: u32, state: u32) {
        self.seat.client.event(Key {
            self_id: self.id,
            serial: serial as _,
            time,
            key,
            state,
        })
    }

    pub fn on_mods_changed(&self, serial: u64, surface: WlSurfaceId, kb_state: &KeyboardState) {
        if self.kb_state_id.get() != kb_state.id {
            self.send_kb_state(serial, kb_state, surface, true);
        } else {
            self.send_modifiers(serial, &kb_state.mods);
        }
    }

    fn send_modifiers(&self, serial: u64, mods: &ModifierState) {
        self.seat.client.event(Modifiers {
            self_id: self.id,
            serial: serial as _,
            mods_depressed: mods.mods_depressed,
            mods_latched: mods.mods_latched,
            mods_locked: mods.mods_locked,
            group: mods.group,
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
    #[error(transparent)]
    XkbCommonError(#[from] XkbCommonError),
}
efrom!(WlKeyboardError, ClientError);
