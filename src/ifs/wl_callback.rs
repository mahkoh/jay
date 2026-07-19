use crate::client::Client;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::object::Version;
use crate::wire::WlCallbackId;
use crate::wire::wl_callback::*;
use std::convert::Infallible;
use std::rc::Rc;

pub struct WlCallback {
    pub client: Rc<Client>,
    pub id: WlCallbackId,
    pub tracker: Tracker<Self>,
}

impl WlCallback {
    pub fn new(id: WlCallbackId, client: &Rc<Client>) -> Self {
        Self {
            client: client.clone(),
            id,
            tracker: Default::default(),
        }
    }

    pub fn send_done(&self, data: u32) {
        self.client.event(Done {
            self_id: self.id,
            callback_data: data,
        });
    }
}

impl WlCallbackRequestHandler for WlCallback {
    type Error = Infallible;
}

object_base! {
    self = WlCallback;
    version = Version(1);
}

impl Object for WlCallback {}

simple_add_obj!(WlCallback);
