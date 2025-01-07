use {
    crate::{
        client::{Client, ClientError},
        clientmem::{ClientMem, ClientMemError},
        ifs::{
            wl_seat::{
                wl_keyboard::{self, WlKeyboard},
                WlSeatGlobal,
            },
            wl_surface::WlSurface,
        },
        kbvm::KbvmError,
        keyboard::KeyboardState,
        leaks::Tracker,
        object::{Object, Version},
        wire::{zwp_virtual_keyboard_v1::*, ZwpVirtualKeyboardV1Id},
    },
    std::{cell::RefCell, rc::Rc},
    thiserror::Error,
};

pub struct ZwpVirtualKeyboardV1 {
    pub id: ZwpVirtualKeyboardV1Id,
    pub client: Rc<Client>,
    pub seat: Rc<WlSeatGlobal>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub kb_state: Rc<RefCell<KeyboardState>>,
}

impl ZwpVirtualKeyboardV1 {
    fn for_each_kb<F>(&self, mut f: F)
    where
        F: FnMut(u64, &WlSurface, &WlKeyboard),
    {
        let Some(surface) = self.seat.keyboard_node.get().node_into_surface() else {
            return;
        };
        let serial = surface.client.next_serial();
        self.seat.surface_kb_event(Version::ALL, &surface, |kb| {
            f(serial, &surface, kb);
        });
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
        let client_mem = ClientMem::new_private(
            &req.fd,
            req.size as usize - 1,
            true,
            Some(&self.client),
            None,
        )
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
            .kb_ctx
            .parse_keymap(&map)
            .map_err(ZwpVirtualKeyboardV1Error::ParseKeymap)?;
        *self.kb_state.borrow_mut() = KeyboardState {
            id: self.client.state.keyboard_state_ids.next(),
            map: map.map.clone(),
            map_len: map.map_len,
            pressed_keys: Default::default(),
            mods: Default::default(),
        };
        Ok(())
    }

    fn key(&self, req: Key, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let kb_state = &mut *self.kb_state.borrow_mut();
        let contains = kb_state.pressed_keys.contains(&req.key);
        let valid = match req.state {
            wl_keyboard::RELEASED => contains,
            wl_keyboard::PRESSED => !contains,
            _ => return Err(ZwpVirtualKeyboardV1Error::UnknownState(req.state)),
        };
        if valid {
            self.for_each_kb(|serial, surface, kb| {
                kb.on_key(serial, req.time, req.key, req.state, surface.id, kb_state);
            });
            match req.state {
                wl_keyboard::RELEASED => kb_state.pressed_keys.remove(&req.key),
                _ => kb_state.pressed_keys.insert(req.key),
            };
            self.seat.latest_kb_state_id.set(kb_state.id);
            self.seat.latest_kb_state.set(self.kb_state.clone());
        }
        Ok(())
    }

    fn modifiers(&self, req: Modifiers, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let kb_state = &mut *self.kb_state.borrow_mut();
        kb_state.mods.mods_pressed.0 = req.mods_depressed;
        kb_state.mods.mods_latched.0 = req.mods_latched;
        kb_state.mods.mods_locked.0 = req.mods_locked;
        kb_state.mods.group_locked.0 = req.group;
        kb_state.mods.update_effective();
        self.for_each_kb(|serial, surface, kb| {
            kb.on_mods_changed(serial, surface.id, &kb_state);
        });
        self.seat.latest_kb_state_id.set(kb_state.id);
        self.seat.latest_kb_state.set(self.kb_state.clone());
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
    ParseKeymap(#[source] KbvmError),
}
efrom!(ZwpVirtualKeyboardV1Error, ClientError);
