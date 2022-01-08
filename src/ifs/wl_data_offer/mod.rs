mod types;

use crate::client::{AddObj, Client};
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
pub use types::*;

const ACCEPT: u32 = 0;
const RECEIVE: u32 = 1;
const DESTROY: u32 = 2;
const FINISH: u32 = 3;
const SET_ACTIONS: u32 = 4;

const OFFER: u32 = 0;
const SOURCE_ACTIONS: u32 = 1;
const ACTION: u32 = 2;

#[allow(dead_code)]
const INVALID_FINISH: u32 = 0;
#[allow(dead_code)]
const INVALID_ACTION_MASK: u32 = 1;
#[allow(dead_code)]
const INVALID_ACTION: u32 = 2;
#[allow(dead_code)]
const INVALID_OFFER: u32 = 3;

id!(WlDataOfferId);

pub struct WlDataOffer {
    id: WlDataOfferId,
    client: Rc<Client>,
}

impl WlDataOffer {
    async fn accept(&self, parser: MsgParser<'_, '_>) -> Result<(), AcceptError> {
        let _req: Accept = self.client.parse(self, parser)?;
        Ok(())
    }

    async fn receive(&self, parser: MsgParser<'_, '_>) -> Result<(), ReceiveError> {
        let _req: Receive = self.client.parse(self, parser)?;
        Ok(())
    }

    async fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self).await?;
        Ok(())
    }

    async fn finish(&self, parser: MsgParser<'_, '_>) -> Result<(), FinishError> {
        let _req: Finish = self.client.parse(self, parser)?;
        Ok(())
    }

    async fn set_actions(&self, parser: MsgParser<'_, '_>) -> Result<(), SetActionsError> {
        let _req: SetActions = self.client.parse(self, parser)?;
        Ok(())
    }

    async fn handle_request_(
        self: &Rc<Self>,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlDataOfferError> {
        match request {
            ACCEPT => self.accept(parser).await?,
            RECEIVE => self.receive(parser).await?,
            DESTROY => self.destroy(parser).await?,
            FINISH => self.finish(parser).await?,
            SET_ACTIONS => self.set_actions(parser).await?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

handle_request!(WlDataOffer);

impl Object for WlDataOffer {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::WlDataOffer
    }

    fn num_requests(&self) -> u32 {
        SET_ACTIONS + 1
    }
}
