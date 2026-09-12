use crate::client::CAP_INPUT_METHOD;
use crate::client::Client;
use crate::client::ClientCaps;
use crate::client::ClientError;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::wl_seat::text_input::zwp_input_method_v2::ZwpInputMethodV2;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::ZwpInputMethodManagerV2Id;
use crate::wire::zwp_input_method_manager_v2::*;
use jay_proc::Object;
use std::rc::Rc;
use thiserror::Error;

pub struct ZwpInputMethodManagerV2Global {
    name: GlobalName,
}

#[derive(Object)]
pub struct ZwpInputMethodManagerV2 {
    id: ZwpInputMethodManagerV2Id,
    client: Rc<Client>,
    tracker: Tracker<Self>,
    version: Version,
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
        client.add_client_obj(&obj);
        Ok(())
    }
}

global_base!(ZwpInputMethodManagerV2Global, ZwpInputMethodManagerV2);

impl Global for ZwpInputMethodManagerV2Global {
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
        self.client.add_client_obj(&im);
        if inert {
            im.send_unavailable();
        } else {
            seat.global.set_input_method(im);
        }
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum ZwpTextInputManagerV3Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZwpTextInputManagerV3Error, ClientError);
