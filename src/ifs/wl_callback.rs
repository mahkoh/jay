use crate::client::Client;
use crate::object::Object;
use crate::wire::wl_callback::*;
use crate::wire::WlCallbackId;
use std::rc::Rc;
use thiserror::Error;

pub struct WlCallback {
    client: Rc<Client>,
    id: WlCallbackId,
}

impl WlCallback {
    pub fn new(id: WlCallbackId, client: &Rc<Client>) -> Self {
        Self {
            client: client.clone(),
            id,
        }
    }

    pub fn send_done(&self) {
        self.client.event(Done {
            self_id: self.id,
            callback_data: 0,
        });
    }
}

object_base! {
    WlCallback, WlCallbackError;
}

impl Object for WlCallback {
    fn num_requests(&self) -> u32 {
        0
    }
}

simple_add_obj!(WlCallback);

#[derive(Debug, Error)]
pub enum WlCallbackError {}
