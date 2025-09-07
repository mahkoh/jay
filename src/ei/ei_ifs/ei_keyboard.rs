use {
    crate::{
        backend::KeyState,
        ei::{
            ei_client::{EiClient, EiClientError},
            ei_ifs::ei_device::{EiDevice, EiDeviceInterface},
            ei_object::{EiObject, EiVersion},
        },
        keyboard::KeyboardState,
        leaks::Tracker,
        wire_ei::{
            EiKeyboardId,
            ei_keyboard::{
                ClientKey, EiKeyboardRequestHandler, Keymap, Modifiers, Release, ServerKey,
            },
        },
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct EiKeyboard {
    pub id: EiKeyboardId,
    pub client: Rc<EiClient>,
    pub tracker: Tracker<Self>,
    pub version: EiVersion,
    pub device: Rc<EiDevice>,
}

ei_device_interface!(EiKeyboard, ei_keyboard, keyboard);

const KEYMAP_TYPE_XKB: u32 = 1;

impl EiKeyboard {
    pub fn send_keymap(&self, state: &KeyboardState) {
        self.client.event(Keymap {
            self_id: self.id,
            keymap_type: KEYMAP_TYPE_XKB,
            size: state.map.map.len as _,
            keymap: state.map.map.map.clone(),
        });
    }

    pub fn send_modifiers(&self, state: &KeyboardState) {
        self.client.event(Modifiers {
            self_id: self.id,
            serial: self.client.serial(),
            depressed: state.mods.mods_pressed.0,
            locked: state.mods.mods_locked.0,
            latched: state.mods.mods_latched.0,
            group: state.mods.group.0,
        });
    }

    pub fn send_key(&self, key: u32, state: KeyState) {
        self.client.event(ServerKey {
            self_id: self.id,
            key,
            state: match state {
                KeyState::Released => 0,
                KeyState::Pressed => 1,
            },
        });
    }
}

impl EiKeyboardRequestHandler for EiKeyboard {
    type Error = EiKeyboardError;

    fn release(&self, _req: Release, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.destroy()?;
        Ok(())
    }

    fn client_key(&self, req: ClientKey, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let pressed = match req.state {
            0 => KeyState::Released,
            1 => KeyState::Pressed,
            _ => return Err(EiKeyboardError::InvalidKeyState(req.state)),
        };
        self.device.key_changes.push((req.key, pressed));
        Ok(())
    }
}

ei_object_base! {
    self = EiKeyboard;
    version = self.version;
}

impl EiObject for EiKeyboard {}

#[derive(Debug, Error)]
pub enum EiKeyboardError {
    #[error(transparent)]
    EiClientError(Box<EiClientError>),
    #[error("Invalid key state {0}")]
    InvalidKeyState(u32),
}
efrom!(EiKeyboardError, EiClientError);
