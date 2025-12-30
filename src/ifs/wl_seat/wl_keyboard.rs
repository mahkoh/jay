use {
    crate::{
        backend::KeyState,
        client::{Client, ClientError, EventFormatter},
        ifs::wl_seat::WlSeat,
        keyboard::{KeyboardError, KeyboardState, KeyboardStateId},
        leaks::Tracker,
        object::{Object, Version},
        state::State,
        utils::{errorfmt::ErrorFmt, vecset::VecSet},
        wire::{WlKeyboardId, WlSurfaceId, wl_keyboard::*},
    },
    kbvm::Components,
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
    thiserror::Error,
};

pub const REPEAT_INFO_SINCE: Version = Version(4);
pub const REPEATED_SINCE: Version = Version(10);
pub const FRAME_SINCE: Version = Version(11);

#[expect(dead_code)]
const NO_KEYMAP: u32 = 0;
pub const XKB_V1: u32 = 1;

pub const RELEASED: u32 = 0;
pub const PRESSED: u32 = 1;
pub const REPEATED: u32 = 2;

pub async fn handle_wl_keyboard_frames(state: Rc<State>) {
    loop {
        let kb = state.keyboard_frames.pop().await;
        if kb.destroyed.get() {
            continue;
        }
        kb.send_frame();
    }
}

pub struct WlKeyboard {
    id: WlKeyboardId,
    client: Rc<Client>,
    version: Version,
    seat: Rc<WlSeat>,
    kb_state_id: Cell<KeyboardStateId>,
    pressed_keys: RefCell<VecSet<u32>>,
    destroyed: Cell<bool>,
    frame_scheduled: Cell<bool>,
    pub tracker: Tracker<Self>,
}

impl WlKeyboard {
    pub fn new(id: WlKeyboardId, seat: &Rc<WlSeat>) -> Self {
        Self {
            id,
            client: seat.client.clone(),
            version: seat.version,
            seat: seat.clone(),
            kb_state_id: Cell::new(KeyboardStateId::from_raw(0)),
            pressed_keys: Default::default(),
            destroyed: Cell::new(false),
            frame_scheduled: Cell::new(false),
            tracker: Default::default(),
        }
    }

    pub fn kb_state_id(&self) -> KeyboardStateId {
        self.kb_state_id.get()
    }

    fn send_kb_state(
        self: &Rc<Self>,
        serial: u64,
        kb_state: &KeyboardState,
        surface_id: WlSurfaceId,
        on_surface: bool,
    ) {
        self.kb_state_id.set(kb_state.id);
        let send_enter;
        if on_surface {
            if self.pressed_keys.borrow().is_empty() && kb_state.pressed_keys.is_empty() {
                send_enter = false;
            } else {
                self.send_leave(serial, surface_id);
                send_enter = true;
            }
        } else {
            send_enter = true;
        }
        self.send_keymap(kb_state);
        if send_enter {
            self.send_enter(serial, surface_id, &kb_state.pressed_keys);
        }
        self.send_modifiers(serial, &kb_state.mods);
    }

    fn send_keymap(self: &Rc<Self>, state: &KeyboardState) {
        let fd = match self.seat.keymap_fd(state) {
            Ok(fd) => fd,
            Err(e) => {
                log::error!(
                    "Could not creat a file descriptor to transfer the keymap to client {}: {}",
                    self.client.id,
                    ErrorFmt(e)
                );
                return;
            }
        };
        self.event(Keymap {
            self_id: self.id,
            format: XKB_V1,
            fd: fd.map,
            size: fd.len as _,
        });
    }

    pub fn enter(self: &Rc<Self>, serial: u64, surface: WlSurfaceId, kb_state: &KeyboardState) {
        if kb_state.id != self.kb_state_id.get() {
            self.send_kb_state(serial, kb_state, surface, false);
        } else {
            self.send_enter(serial, surface, &kb_state.pressed_keys);
            self.send_modifiers(serial, &kb_state.mods);
        }
    }

    fn send_enter(self: &Rc<Self>, serial: u64, surface: WlSurfaceId, keys: &[u32]) {
        {
            let pk = &mut self.pressed_keys.borrow_mut();
            pk.clear();
            pk.extend(keys);
        }
        self.event(Enter {
            self_id: self.id,
            serial: serial as _,
            surface,
            keys,
        });
    }

    pub fn send_leave(self: &Rc<Self>, serial: u64, surface: WlSurfaceId) {
        self.event(Leave {
            self_id: self.id,
            serial: serial as _,
            surface,
        });
    }

    pub fn on_key(
        self: &Rc<Self>,
        serial: u64,
        time: u32,
        key: u32,
        state: KeyState,
        surface: WlSurfaceId,
        kb_state: &KeyboardState,
    ) {
        if self.kb_state_id.get() != kb_state.id {
            self.send_kb_state(serial, kb_state, surface, true);
        }
        self.send_key(serial, time, key, state);
    }

    fn send_key(self: &Rc<Self>, serial: u64, time: u32, key: u32, state: KeyState) {
        if state == KeyState::Repeated && self.version < REPEATED_SINCE {
            return;
        }
        {
            let pk = &mut self.pressed_keys.borrow_mut();
            match state {
                KeyState::Released => {
                    if !pk.remove(&key) {
                        return;
                    }
                }
                KeyState::Pressed => {
                    if !pk.insert(key) {
                        return;
                    }
                }
                KeyState::Repeated => {
                    if !pk.contains(&key) {
                        return;
                    }
                }
            }
        }
        self.event(Key {
            self_id: self.id,
            serial: serial as _,
            time,
            key,
            state: match state {
                KeyState::Released => RELEASED,
                KeyState::Pressed => PRESSED,
                KeyState::Repeated => REPEATED,
            },
        });
    }

    pub fn on_mods_changed(
        self: &Rc<Self>,
        serial: u64,
        surface: WlSurfaceId,
        kb_state: &KeyboardState,
    ) {
        if self.kb_state_id.get() != kb_state.id {
            self.send_kb_state(serial, kb_state, surface, true);
        } else {
            self.send_modifiers(serial, &kb_state.mods);
        }
    }

    fn send_modifiers(self: &Rc<Self>, serial: u64, mods: &Components) {
        self.event(Modifiers {
            self_id: self.id,
            serial: serial as _,
            mods_depressed: mods.mods_pressed.0,
            mods_latched: mods.mods_latched.0,
            mods_locked: mods.mods_locked.0,
            group: mods.group.0,
        });
    }

    pub fn send_repeat_info(self: &Rc<Self>, mut rate: i32, mut delay: i32) {
        if self.version >= REPEATED_SINCE {
            rate = 0;
            delay = 0;
        }
        self.event(RepeatInfo {
            self_id: self.id,
            rate,
            delay,
        });
    }

    fn event<T: EventFormatter>(self: &Rc<Self>, event: T) {
        self.client.event(event);
        if self.version < FRAME_SINCE {
            return;
        }
        if self.frame_scheduled.replace(true) {
            return;
        }
        self.client.state.keyboard_frames.push(self.clone());
    }

    fn send_frame(&self) {
        self.frame_scheduled.set(false);
        self.seat.client.event(Frame { self_id: self.id });
    }
}

impl WlKeyboardRequestHandler for WlKeyboard {
    type Error = WlKeyboardError;

    fn release(&self, _req: Release, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.destroyed.set(true);
        self.seat.keyboards.remove(&self.id);
        self.seat.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = WlKeyboard;
    version = self.version;
}

impl Object for WlKeyboard {}

simple_add_obj!(WlKeyboard);

#[derive(Debug, Error)]
pub enum WlKeyboardError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    KeyboardError(#[from] KeyboardError),
}
efrom!(WlKeyboardError, ClientError);
