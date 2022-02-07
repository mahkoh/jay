use crate::client::{Client, ClientError};
use crate::object::Object;
use crate::utils::buffd::{MsgParser, MsgParserError};
use crate::wire::zwp_primary_selection_source_v1::*;
use crate::wire::ZwpPrimarySelectionSourceV1Id;
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;
use crate::ifs::ipc::{add_mime_type, disconnect_source, SourceData};
use crate::ifs::ipc::zwp_primary_selection_device_v1::ZwpPrimarySelectionDeviceV1;

pub struct ZwpPrimarySelectionSourceV1 {
    pub id: ZwpPrimarySelectionSourceV1Id,
    pub data: SourceData<ZwpPrimarySelectionDeviceV1>,
}

impl ZwpPrimarySelectionSourceV1 {
    pub fn new(id: ZwpPrimarySelectionSourceV1Id, client: &Rc<Client>) -> Self {
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
        add_mime_type::<ZwpPrimarySelectionDeviceV1>(self, req.mime_type);
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.data.client.parse(self, parser)?;
        disconnect_source::<ZwpPrimarySelectionDeviceV1>(self);
        self.data.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    ZwpPrimarySelectionSourceV1, ZwpPrimarySelectionSourceV1Error;

    OFFER => offer,
    DESTROY => destroy,
}

impl Object for ZwpPrimarySelectionSourceV1 {
    fn num_requests(&self) -> u32 {
        DESTROY + 1
    }

    fn break_loops(&self) {
        disconnect_source::<ZwpPrimarySelectionDeviceV1>(self);
    }
}

dedicated_add_obj!(
    ZwpPrimarySelectionSourceV1,
    ZwpPrimarySelectionSourceV1Id,
    zwp_primary_selection_source
);

#[derive(Debug, Error)]
pub enum ZwpPrimarySelectionSourceV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process `offer` request")]
    OfferError(#[from] OfferError),
    #[error("Could not process `destroy` request")]
    DestroyError(#[from] DestroyError),
}
efrom!(ZwpPrimarySelectionSourceV1Error, ClientError);

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
