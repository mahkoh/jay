use {
    crate::{
        backend::KeyState,
        ei::{
            ei_client::{EiClient, EiClientError},
            ei_ifs::ei_device::{EiDevice, EiDeviceInterface},
            ei_object::{EiObject, EiVersion},
        },
        leaks::Tracker,
        wire_ei::{
            ei_keyboard::{
                ClientKey, EiKeyboardRequestHandler, Keymap, Modifiers, Release, ServerKey,
            },
            EiKeyboardId,
        },
        xkbcommon::KeyboardState,
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
            size: state.map_len as _,
            keymap: state.map.clone(),
        });
    }

    pub fn send_modifiers(&self, state: &KeyboardState) {
        self.client.event(Modifiers {
            self_id: self.id,
            serial: self.client.serial(),
            depressed: state.mods.mods_depressed,
            locked: state.mods.mods_locked,
            latched: state.mods.mods_latched,
            group: state.mods.group,
        });
    }

    pub fn send_key(&self, key: u32, state: u32) {
        self.client.event(ServerKey {
            self_id: self.id,
            key,
            state,
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
