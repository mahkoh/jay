use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_seat::text_input::zwp_text_input_v3::ZwpTextInputV3,
        leaks::Tracker,
        object::{Object, Version},
        wire::{zwp_text_input_manager_v3::*, ZwpTextInputManagerV3Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwpTextInputManagerV3Global {
    pub name: GlobalName,
}

pub struct ZwpTextInputManagerV3 {
    pub id: ZwpTextInputManagerV3Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl ZwpTextInputManagerV3Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ZwpTextInputManagerV3Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), ZwpTextInputManagerV3Error> {
        let obj = Rc::new(ZwpTextInputManagerV3 {
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
    ZwpTextInputManagerV3Global,
    ZwpTextInputManagerV3,
    ZwpTextInputManagerV3Error
);

impl Global for ZwpTextInputManagerV3Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(ZwpTextInputManagerV3Global);

impl ZwpTextInputManagerV3RequestHandler for ZwpTextInputManagerV3 {
    type Error = ZwpTextInputManagerV3Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_text_input(&self, req: GetTextInput, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let seat = self.client.lookup(req.seat)?;
        let ti = Rc::new(ZwpTextInputV3::new(
            req.id,
            &self.client,
            &seat.global,
            self.version,
        ));
        track!(self.client, ti);
        self.client.add_client_obj(&ti)?;
        seat.global
            .text_inputs
            .borrow_mut()
            .entry(self.client.id)
            .or_default()
            .set(req.id, ti.clone());
        if let Some(surface) = seat.global.keyboard_node.get().node_into_surface() {
            if surface.client.id == self.client.id {
                ti.send_enter(&surface);
                ti.send_done();
            }
        }
        Ok(())
    }
}

object_base! {
    self = ZwpTextInputManagerV3;
    version = self.version;
}

impl Object for ZwpTextInputManagerV3 {}

simple_add_obj!(ZwpTextInputManagerV3);

#[derive(Debug, Error)]
pub enum ZwpTextInputManagerV3Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpTextInputManagerV3Error, ClientError);
