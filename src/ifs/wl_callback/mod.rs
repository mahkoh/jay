mod types;

use crate::objects::{Interface, Object, ObjectError, ObjectId};
use crate::utils::buffd::WlParser;
use crate::wl_client::DynEventFormatter;
use std::rc::Rc;
use types::*;

const DONE: u32 = 0;

pub struct WlCallback {
    id: ObjectId,
}

impl WlCallback {
    pub fn new(id: ObjectId) -> Self {
        Self { id }
    }

    pub fn done(self: &Rc<Self>) -> DynEventFormatter {
        Box::new(Done { obj: self.clone() })
    }

    async fn handle_request_(
        &self,
        _request: u32,
        _parser: WlParser<'_, '_>,
    ) -> Result<(), ObjectError> {
        unreachable!();
    }
}

handle_request!(WlCallback);

impl Object for WlCallback {
    fn id(&self) -> ObjectId {
        self.id
    }

    fn interface(&self) -> Interface {
        Interface::WlCallback
    }

    fn num_requests(&self) -> u32 {
        0
    }
}
