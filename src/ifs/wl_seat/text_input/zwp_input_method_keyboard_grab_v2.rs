use {
    crate::{
        backend::KeyState,
        client::{Client, ClientError},
        ifs::wl_seat::{text_input::zwp_input_method_v2::ZwpInputMethodV2, wl_keyboard},
        keyboard::{KeyboardState, KeyboardStateId},
        leaks::Tracker,
        object::{Object, Version},
        utils::errorfmt::ErrorFmt,
        wire::{zwp_input_method_keyboard_grab_v2::*, ZwpInputMethodKeyboardGrabV2Id},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct ZwpInputMethodKeyboardGrabV2 {
    pub id: ZwpInputMethodKeyboardGrabV2Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub input_method: Rc<ZwpInputMethodV2>,
    pub kb_state_id: Cell<KeyboardStateId>,
}

impl ZwpInputMethodKeyboardGrabV2 {
    fn detach(&self) {
        self.input_method.seat.input_method_grab.take();
    }

    fn send_keymap(&self, kb_state: &KeyboardState) {
        let map = match kb_state.map.create_unprotected_fd() {
            Ok(m) => m,
            Err(e) => {
                log::error!("Could not create new keymap fd: {}", ErrorFmt(e));
                return;
            }
        };
        self.client.event(Keymap {
            self_id: self.id,
            format: wl_keyboard::XKB_V1,
            fd: map.map,
            size: map.len as _,
        });
    }

    fn update_state(&self, serial: u64, kb_state: &KeyboardState) {
        self.send_keymap(kb_state);
        self.send_modifiers(serial, kb_state);
        self.kb_state_id.set(kb_state.id);
    }

    pub fn on_key(&self, time_usec: u64, key: u32, state: KeyState, kb_state: &KeyboardState) {
        let serial = self.client.next_serial();
        if self.kb_state_id.get() != kb_state.id {
            self.update_state(serial, kb_state);
        }
        self.send_key(serial, time_usec, key, state);
    }

    fn send_key(&self, serial: u64, time_usec: u64, key: u32, state: KeyState) {
        self.client.event(Key {
            self_id: self.id,
            serial: serial as _,
            time: (time_usec / 1000) as _,
            key,
            state: match state {
                KeyState::Released => wl_keyboard::RELEASED,
                KeyState::Pressed => wl_keyboard::PRESSED,
            },
        })
    }

    pub fn on_modifiers(&self, kb_state: &KeyboardState) {
        let serial = self.client.next_serial();
        if self.kb_state_id.get() != kb_state.id {
            self.update_state(serial, kb_state);
        }
        self.send_modifiers(serial, kb_state);
    }

    fn send_modifiers(&self, serial: u64, kb_state: &KeyboardState) {
        self.client.event(Modifiers {
            self_id: self.id,
            serial: serial as _,
            mods_depressed: kb_state.mods.mods_pressed.0,
            mods_latched: kb_state.mods.mods_latched.0,
            mods_locked: kb_state.mods.mods_locked.0,
            group: kb_state.mods.group.0,
        })
    }

    pub fn send_repeat_info(&self) {
        let (rate, delay) = self.input_method.seat.repeat_rate.get();
        self.client.event(RepeatInfo {
            self_id: self.id,
            rate,
            delay,
        })
    }
}

impl ZwpInputMethodKeyboardGrabV2RequestHandler for ZwpInputMethodKeyboardGrabV2 {
    type Error = ZwpInputMethodKeyboardGrabV2Error;

    fn release(&self, _req: Release, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwpInputMethodKeyboardGrabV2;
    version = self.version;
}

impl Object for ZwpInputMethodKeyboardGrabV2 {
    fn break_loops(&self) {
        self.detach();
    }
}

simple_add_obj!(ZwpInputMethodKeyboardGrabV2);

#[derive(Debug, Error)]
pub enum ZwpInputMethodKeyboardGrabV2Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpInputMethodKeyboardGrabV2Error, ClientError);
