use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_seat::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
        leaks::Tracker,
        object::{Object, Version},
        wire::{zwp_virtual_keyboard_manager_v1::*, ZwpVirtualKeyboardManagerV1Id},
        xkbcommon::KeyboardState,
    },
    std::{cell::RefCell, rc::Rc},
    thiserror::Error,
};

pub struct ZwpVirtualKeyboardManagerV1Global {
    pub name: GlobalName,
}

pub struct ZwpVirtualKeyboardManagerV1 {
    pub id: ZwpVirtualKeyboardManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl ZwpVirtualKeyboardManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ZwpVirtualKeyboardManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), ZwpVirtualKeyboardManagerV1Error> {
        let obj = Rc::new(ZwpVirtualKeyboardManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

global_base!(
    ZwpVirtualKeyboardManagerV1Global,
    ZwpVirtualKeyboardManagerV1,
    ZwpVirtualKeyboardManagerV1Error
);

impl Global for ZwpVirtualKeyboardManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }

    fn secure(&self) -> bool {
        true
    }
}

simple_add_global!(ZwpVirtualKeyboardManagerV1Global);

impl ZwpVirtualKeyboardManagerV1RequestHandler for ZwpVirtualKeyboardManagerV1 {
    type Error = ZwpVirtualKeyboardManagerV1Error;

    fn create_virtual_keyboard(
        &self,
        req: CreateVirtualKeyboard,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let seat = self.client.lookup(req.seat)?;
        let seat_keymap = seat.global.seat_kb_map.get();
        let kb = Rc::new(ZwpVirtualKeyboardV1 {
            id: req.id,
            client: self.client.clone(),
            seat: seat.global.clone(),
            tracker: Default::default(),
            version: self.version,
            kb_state: Rc::new(RefCell::new(KeyboardState {
                id: self.client.state.keyboard_state_ids.next(),
                map: seat_keymap.map.clone(),
                map_len: seat_keymap.map_len,
                pressed_keys: Default::default(),
                mods: Default::default(),
            })),
        });
        track!(self.client, kb);
        self.client.add_client_obj(&kb)?;
        Ok(())
    }
}

object_base! {
    self = ZwpVirtualKeyboardManagerV1;
    version = self.version;
}

impl Object for ZwpVirtualKeyboardManagerV1 {}

simple_add_obj!(ZwpVirtualKeyboardManagerV1);

#[derive(Debug, Error)]
pub enum ZwpVirtualKeyboardManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpVirtualKeyboardManagerV1Error, ClientError);
