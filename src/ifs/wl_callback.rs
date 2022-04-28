use {
    crate::{
        client::Client,
        leaks::Tracker,
        object::Object,
        wire::{wl_callback::*, WlCallbackId},
    },
    std::rc::Rc,
    thiserror::Error,
};

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

    pub fn send_done(&self) {
        self.client.event(Done {
            self_id: self.id,
            callback_data: 0,
        });
    }
}

object_base! {
    WlCallback;
}

impl Object for WlCallback {
    fn num_requests(&self) -> u32 {
        0
    }
}

simple_add_obj!(WlCallback);

#[derive(Debug, Error)]
pub enum WlCallbackError {}
