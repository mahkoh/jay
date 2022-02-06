use crate::client::DynEventFormatter;
use crate::object::Object;
use std::rc::Rc;
use thiserror::Error;
use crate::wire::wl_callback::*;
use crate::wire::WlCallbackId;

pub struct WlCallback {
    id: WlCallbackId,
}

impl WlCallback {
    pub fn new(id: WlCallbackId) -> Self {
        Self { id }
    }

    pub fn done(self: &Rc<Self>) -> DynEventFormatter {
        Box::new(Done { self_id: self.id, callback_data: 0 })
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
