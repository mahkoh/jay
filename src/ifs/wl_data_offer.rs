
use crate::client::{Client, ClientError, DynEventFormatter};
use crate::ifs::wl_data_source::WlDataSource;
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::object::Object;
use crate::utils::buffd::MsgParser;
use crate::utils::clonecell::CloneCell;
use std::ops::Deref;
use std::rc::Rc;
use thiserror::Error;
use crate::wire::wl_data_offer::*;
use crate::utils::buffd::MsgParserError;
use crate::wire::WlDataOfferId;

#[allow(dead_code)]
const INVALID_FINISH: u32 = 0;
#[allow(dead_code)]
const INVALID_ACTION_MASK: u32 = 1;
#[allow(dead_code)]
const INVALID_ACTION: u32 = 2;
#[allow(dead_code)]
const INVALID_OFFER: u32 = 3;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum DataOfferRole {
    Selection,
}

pub struct WlDataOffer {
    pub id: WlDataOfferId,
    pub client: Rc<Client>,
    pub role: DataOfferRole,
    pub source: CloneCell<Option<Rc<WlDataSource>>>,
}

impl WlDataOffer {
    pub fn create(
        client: &Rc<Client>,
        role: DataOfferRole,
        src: &Rc<WlDataSource>,
        seat: &Rc<WlSeatGlobal>,
    ) -> Option<Rc<Self>> {
        let id = match client.new_id() {
            Ok(id) => id,
            Err(e) => {
                client.error(e);
                return None;
            }
        };
        let slf = Rc::new(Self {
            id,
            client: client.clone(),
            role,
            source: CloneCell::new(Some(src.clone())),
        });
        let mt = src.mime_types.borrow_mut();
        seat.for_each_data_device(0, client.id, |device| {
            client.event(device.data_offer(slf.id));
            for mt in mt.deref() {
                client.event(slf.offer(mt));
            }
            let ev = match role {
                DataOfferRole::Selection => device.selection(id),
            };
            client.event(ev);
        });
        client.add_server_obj(&slf);
        Some(slf)
    }

    pub fn offer(self: &Rc<Self>, mime_type: &str) -> DynEventFormatter {
        Box::new(OfferOut {
            self_id: self.id,
            mime_type: mime_type.to_string(),
        })
    }

    fn accept(&self, parser: MsgParser<'_, '_>) -> Result<(), AcceptError> {
        let _req: AcceptIn = self.client.parse(self, parser)?;
        Ok(())
    }

    fn receive(&self, parser: MsgParser<'_, '_>) -> Result<(), ReceiveError> {
        let req: ReceiveIn = self.client.parse(self, parser)?;
        if let Some(src) = self.source.get() {
            src.client.event(src.send(req.mime_type, req.fd));
            src.client.flush();
        }
        Ok(())
    }

    fn disconnect(&self) {
        if let Some(src) = self.source.set(None) {
            src.destroy_offer();
        }
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.disconnect();
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
        self.disconnect();
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
