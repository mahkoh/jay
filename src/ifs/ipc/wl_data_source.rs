use crate::client::{Client, ClientError};
use crate::object::Object;
use crate::utils::buffd::MsgParser;
use crate::utils::buffd::MsgParserError;
use crate::wire::wl_data_source::*;
use crate::wire::{WlDataSourceId};
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;
use crate::ifs::ipc::{add_mime_type, disconnect_source, SourceData};
use crate::ifs::ipc::wl_data_device::WlDataDevice;

#[allow(dead_code)]
const INVALID_ACTION_MASK: u32 = 0;
#[allow(dead_code)]
const INVALID_SOURCE: u32 = 1;

pub struct WlDataSource {
    pub id: WlDataSourceId,
    pub data: SourceData<WlDataDevice>,
}

impl WlDataSource {
    pub fn new(id: WlDataSourceId, client: &Rc<Client>) -> Self {
        Self {
            id,
            data: SourceData::new(client),
        }
    }

    pub fn send_cancelled(&self) {
        self.data.client.event(Cancelled { self_id: self.id })
    }

    pub fn send_send(&self, mime_type: &str, fd: Rc<OwnedFd>) {
        self.data.client.event(Send {
            self_id: self.id,
            mime_type,
            fd,
        })
    }

    fn offer(&self, parser: MsgParser<'_, '_>) -> Result<(), OfferError> {
        let req: Offer = self.data.client.parse(self, parser)?;
        add_mime_type::<WlDataDevice>(self, req.mime_type);
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.data.client.parse(self, parser)?;
        disconnect_source::<WlDataDevice>(self);
        self.data.client.remove_obj(self)?;
        Ok(())
    }

    fn set_actions(&self, parser: MsgParser<'_, '_>) -> Result<(), SetActionsError> {
        let _req: SetActions = self.data.client.parse(self, parser)?;
        Ok(())
    }
}

object_base! {
    WlDataSource, WlDataSourceError;

    OFFER => offer,
    DESTROY => destroy,
    SET_ACTIONS => set_actions,
}

impl Object for WlDataSource {
    fn num_requests(&self) -> u32 {
        SET_ACTIONS + 1
    }

    fn break_loops(&self) {
        disconnect_source::<WlDataDevice>(self);
    }
}

dedicated_add_obj!(WlDataSource, WlDataSourceId, wl_data_source);

#[derive(Debug, Error)]
pub enum WlDataSourceError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process `offer` request")]
    OfferError(#[from] OfferError),
    #[error("Could not process `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process `set_actions` request")]
    SetActionsError(#[from] SetActionsError),
}
efrom!(WlDataSourceError, ClientError);

#[derive(Debug, Error)]
pub enum OfferError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(OfferError, ParseFailed, MsgParserError);
efrom!(OfferError, ClientError);

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
pub enum SetActionsError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(SetActionsError, ParseFailed, MsgParserError);
efrom!(SetActionsError, ClientError);
