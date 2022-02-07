use crate::client::{Client, ClientError};
use crate::object::Object;
use crate::utils::buffd::MsgParser;
use crate::utils::buffd::MsgParserError;
use crate::wire::wl_data_offer::*;
use crate::wire::{WlDataOfferId};
use std::rc::Rc;
use thiserror::Error;
use crate::ifs::ipc::{disconnect_offer, OfferData, receive};
use crate::ifs::ipc::wl_data_device::WlDataDevice;

#[allow(dead_code)]
const INVALID_FINISH: u32 = 0;
#[allow(dead_code)]
const INVALID_ACTION_MASK: u32 = 1;
#[allow(dead_code)]
const INVALID_ACTION: u32 = 2;
#[allow(dead_code)]
const INVALID_OFFER: u32 = 3;


pub struct WlDataOffer {
    pub id: WlDataOfferId,
    pub client: Rc<Client>,
    pub offer_data: OfferData<WlDataDevice>,
}

impl WlDataOffer {
    pub fn send_offer(&self, mime_type: &str) {
        self.client.event(Offer {
            self_id: self.id,
            mime_type,
        })
    }

    fn accept(&self, parser: MsgParser<'_, '_>) -> Result<(), AcceptError> {
        let _req: Accept = self.client.parse(self, parser)?;
        Ok(())
    }

    fn receive(&self, parser: MsgParser<'_, '_>) -> Result<(), ReceiveError> {
        let req: Receive = self.client.parse(self, parser)?;
        receive::<WlDataDevice>(self, req.mime_type, req.fd);
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        disconnect_offer::<WlDataDevice>(self);
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn finish(&self, parser: MsgParser<'_, '_>) -> Result<(), FinishError> {
        let _req: Finish = self.client.parse(self, parser)?;
        Ok(())
    }

    fn set_actions(&self, parser: MsgParser<'_, '_>) -> Result<(), SetActionsError> {
        let _req: SetActions = self.client.parse(self, parser)?;
        Ok(())
    }
}

object_base! {
    WlDataOffer, WlDataOfferError;

    ACCEPT => accept,
    RECEIVE => receive,
    DESTROY => destroy,
    FINISH => finish,
    SET_ACTIONS => set_actions,
}

impl Object for WlDataOffer {
    fn num_requests(&self) -> u32 {
        SET_ACTIONS + 1
    }

    fn break_loops(&self) {
        disconnect_offer::<WlDataDevice>(self);
    }
}

simple_add_obj!(WlDataOffer);

#[derive(Debug, Error)]
pub enum WlDataOfferError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process `accept` request")]
    AcceptError(#[from] AcceptError),
    #[error("Could not process `receive` request")]
    ReceiveError(#[from] ReceiveError),
    #[error("Could not process `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process `finish` request")]
    FinishError(#[from] FinishError),
    #[error("Could not process `set_actions` request")]
    SetActionsError(#[from] SetActionsError),
}
efrom!(WlDataOfferError, ClientError);

#[derive(Debug, Error)]
pub enum AcceptError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(AcceptError, ParseFailed, MsgParserError);
efrom!(AcceptError, ClientError);

#[derive(Debug, Error)]
pub enum ReceiveError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ReceiveError, ParseFailed, MsgParserError);
efrom!(ReceiveError, ClientError);

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ParseFailed, MsgParserError);
efrom!(DestroyError, ClientError);

#[derive(Debug, Error)]
pub enum FinishError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(FinishError, ParseFailed, MsgParserError);
efrom!(FinishError, ClientError);

#[derive(Debug, Error)]
pub enum SetActionsError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(SetActionsError, ParseFailed, MsgParserError);
efrom!(SetActionsError, ClientError);
