use {
    crate::{
        client::{CAP_INPUT_METHOD, Client, ClientCaps, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_seat::text_input::zwp_input_method_v2::ZwpInputMethodV2,
        leaks::Tracker,
        object::{Object, Version},
        wire::{ZwpInputMethodManagerV2Id, zwp_input_method_manager_v2::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwpInputMethodManagerV2Global {
    pub name: GlobalName,
}

pub struct ZwpInputMethodManagerV2 {
    pub id: ZwpInputMethodManagerV2Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl ZwpInputMethodManagerV2Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: ZwpInputMethodManagerV2Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), ZwpTextInputManagerV3Error> {
        let obj = Rc::new(ZwpInputMethodManagerV2 {
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
    ZwpInputMethodManagerV2Global,
    ZwpInputMethodManagerV2,
    ZwpTextInputManagerV3Error
);

impl Global for ZwpInputMethodManagerV2Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }

    fn required_caps(&self) -> ClientCaps {
        CAP_INPUT_METHOD
    }
}

simple_add_global!(ZwpInputMethodManagerV2Global);

impl ZwpInputMethodManagerV2RequestHandler for ZwpInputMethodManagerV2 {
    type Error = ZwpTextInputManagerV3Error;

    fn get_input_method(&self, req: GetInputMethod, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let seat = self.client.lookup(req.seat)?;
        let inert = seat.global.cannot_set_new_im();
        let im = Rc::new(ZwpInputMethodV2 {
            id: req.input_method,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            seat: seat.global.clone(),
            popups: Default::default(),
            connection: Default::default(),
            inert,
            num_done: Default::default(),
            pending: Default::default(),
        });
        track!(self.client, im);
        self.client.add_client_obj(&im)?;
        if inert {
            im.send_unavailable();
        } else {
            seat.global.set_input_method(im);
        }
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = ZwpInputMethodManagerV2;
    version = self.version;
}

impl Object for ZwpInputMethodManagerV2 {}

simple_add_obj!(ZwpInputMethodManagerV2);

#[derive(Debug, Error)]
pub enum ZwpTextInputManagerV3Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpTextInputManagerV3Error, ClientError);
