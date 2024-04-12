use {
    crate::{
        backend::KeyState,
        client::{Client, ClientError},
        clientmem::{ClientMem, ClientMemError},
        ifs::wl_seat::{wl_keyboard, WlSeatGlobal},
        leaks::Tracker,
        object::{Object, Version},
        utils::clonecell::CloneCell,
        wire::{zwp_virtual_keyboard_v1::*, ZwpVirtualKeyboardV1Id},
        xkbcommon::{KeymapId, XkbCommonError, XkbKeymap},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct ZwpVirtualKeyboardV1 {
    pub id: ZwpVirtualKeyboardV1Id,
    pub client: Rc<Client>,
    pub seat: Rc<WlSeatGlobal>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub keymap_id: Cell<Option<KeymapId>>,
    pub keymap: CloneCell<Option<Rc<XkbKeymap>>>,
}

impl ZwpVirtualKeyboardV1 {
    fn ensure_keymap(&self) {
        if let Some(id) = self.keymap_id.get() {
            if id == self.seat.effective_kb_map_id.get() {
                return;
            }
        }
        let Some(keymap) = self.keymap.get() else {
            return;
        };
        self.seat.set_effective_keymap(&keymap);
    }
}

impl ZwpVirtualKeyboardV1RequestHandler for ZwpVirtualKeyboardV1 {
    type Error = ZwpVirtualKeyboardV1Error;

    fn keymap(&self, req: Keymap, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if req.format != wl_keyboard::XKB_V1 {
            return Err(ZwpVirtualKeyboardV1Error::UnsupportedFormat(req.format));
        }
        if req.size == 0 {
            return Err(ZwpVirtualKeyboardV1Error::InvalidKeymap);
        }
        const MAX_SIZE: u32 = 1024 * 1024;
        if req.size > MAX_SIZE {
            return Err(ZwpVirtualKeyboardV1Error::OversizedKeymap);
        }
        let client_mem = ClientMem::new(req.fd.raw(), req.size as usize - 1, true)
            .map(Rc::new)
            .map_err(ZwpVirtualKeyboardV1Error::MapKeymap)?;
        let mut map = vec![];
        client_mem
            .offset(0)
            .read(&mut map)
            .map_err(ZwpVirtualKeyboardV1Error::ReadKeymap)?;
        let map = self
            .client
            .state
            .xkb_ctx
            .keymap_from_str(&map)
            .map_err(ZwpVirtualKeyboardV1Error::ParseKeymap)?;
        self.keymap_id.set(Some(map.id));
        self.keymap.set(Some(map));
        Ok(())
    }

    fn key(&self, req: Key, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.ensure_keymap();
        let time_usec = (req.time as u64) * 1000;
        let state = match req.state {
            wl_keyboard::RELEASED => KeyState::Released,
            wl_keyboard::PRESSED => KeyState::Pressed,
            _ => return Err(ZwpVirtualKeyboardV1Error::UnknownState(req.state)),
        };
        self.seat.key_event(time_usec, req.key, state);
        Ok(())
    }

    fn modifiers(&self, req: Modifiers, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.ensure_keymap();
        self.seat.set_modifiers(
            req.mods_depressed,
            req.mods_latched,
            req.mods_locked,
            req.group,
        );
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwpVirtualKeyboardV1;
    version = self.version;
}

impl Object for ZwpVirtualKeyboardV1 {}

simple_add_obj!(ZwpVirtualKeyboardV1);

#[derive(Debug, Error)]
pub enum ZwpVirtualKeyboardV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Unknown key state {0}")]
    UnknownState(u32),
    #[error("Unsupported keymap format {0}")]
    UnsupportedFormat(u32),
    #[error("Keymap is invalid")]
    InvalidKeymap,
    #[error("Keymap is too large")]
    OversizedKeymap,
    #[error("Could not map the keymap")]
    MapKeymap(#[source] ClientMemError),
    #[error("Could not read the keymap")]
    ReadKeymap(#[source] ClientMemError),
    #[error("Could not parse the keymap")]
    ParseKeymap(#[source] XkbCommonError),
}
efrom!(ZwpVirtualKeyboardV1Error, ClientError);
