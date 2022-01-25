mod types;

use crate::client::Client;
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
pub use types::*;

const OFFER: u32 = 0;
const DESTROY: u32 = 1;
const SET_ACTIONS: u32 = 2;

const TARGET: u32 = 0;
const SEND: u32 = 1;
const CANCELLED: u32 = 2;
const DND_DROP_PERFORMED: u32 = 4;
const DND_FINISHED: u32 = 5;
const ACTION: u32 = 5;

#[allow(dead_code)]
const INVALID_ACTION_MASK: u32 = 0;
#[allow(dead_code)]
const INVALID_SOURCE: u32 = 1;

id!(WlDataSourceId);

pub struct WlDataSource {
    id: WlDataSourceId,
    client: Rc<Client>,
}

impl WlDataSource {
    pub fn new(id: WlDataSourceId, client: &Rc<Client>) -> Self {
        Self {
            id,
            client: client.clone(),
        }
    }

    fn offer(&self, parser: MsgParser<'_, '_>) -> Result<(), OfferError> {
        let _req: Offer = self.client.parse(self, parser)?;
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_actions(&self, parser: MsgParser<'_, '_>) -> Result<(), SetActionsError> {
        let _req: SetActions = self.client.parse(self, parser)?;
        Ok(())
    }

    fn handle_request_(
        self: &Rc<Self>,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlDataSourceError> {
        match request {
            OFFER => self.offer(parser)?,
            DESTROY => self.destroy(parser)?,
            SET_ACTIONS => self.set_actions(parser)?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

handle_request!(WlDataSource);

impl Object for WlDataSource {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::WlDataSource
    }

    fn num_requests(&self) -> u32 {
        SET_ACTIONS + 1
    }
}
