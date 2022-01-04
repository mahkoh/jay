mod types;

use crate::client::{ClientError, DynEventFormatter};
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
use types::*;

const DONE: u32 = 0;

id!(WlCallbackId);

pub struct WlCallback {
    id: WlCallbackId,
}

impl WlCallback {
    pub fn new(id: WlCallbackId) -> Self {
        Self { id }
    }

    pub fn done(self: &Rc<Self>) -> DynEventFormatter {
        Box::new(Done { obj: self.clone() })
    }

    async fn handle_request_(
        &self,
        _request: u32,
        _parser: MsgParser<'_, '_>,
    ) -> Result<(), ClientError> {
        unreachable!();
    }
}

handle_request!(WlCallback);

impl Object for WlCallback {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::WlCallback
    }

    fn num_requests(&self) -> u32 {
        0
    }
}
