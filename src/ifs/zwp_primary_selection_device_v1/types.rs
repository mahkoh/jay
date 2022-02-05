use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::ifs::zwp_primary_selection_device_v1::{
    ZwpPrimarySelectionDeviceV1, DATA_OFFER, SELECTION,
};
use crate::ifs::zwp_primary_selection_offer_v1::ZwpPrimarySelectionOfferV1Id;
use crate::ifs::zwp_primary_selection_source_v1::{
    ZwpPrimarySelectionSourceV1Error, ZwpPrimarySelectionSourceV1Id,
};
use crate::object::Object;
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ZwpPrimarySelectionDeviceV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process `set_selection` request")]
    SetSelectionError(#[from] SetSelectionError),
    #[error("Could not process `destroy` request")]
    DestroyError(#[from] DestroyError),
}
efrom!(ZwpPrimarySelectionDeviceV1Error, ClientError);

#[derive(Debug, Error)]
pub enum SetSelectionError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    ZwpPrimarySelectionSourceV1Error(Box<ZwpPrimarySelectionSourceV1Error>),
}
efrom!(SetSelectionError, ParseFailed, MsgParserError);
efrom!(SetSelectionError, ClientError);
efrom!(SetSelectionError, ZwpPrimarySelectionSourceV1Error);

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ParseFailed, MsgParserError);
efrom!(DestroyError, ClientError);

pub(super) struct SetSelection {
    pub source: ZwpPrimarySelectionSourceV1Id,
    pub serial: u32,
}
impl RequestParser<'_> for SetSelection {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            source: parser.object()?,
            serial: parser.uint()?,
        })
    }
}
impl Debug for SetSelection {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "set_selection(source: {}, serial: {})",
            self.source, self.serial,
        )
    }
}

pub(super) struct Destroy;
impl RequestParser<'_> for Destroy {
    fn parse(_parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self)
    }
}
impl Debug for Destroy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "destroy()")
    }
}

pub(super) struct DataOffer {
    pub obj: Rc<ZwpPrimarySelectionDeviceV1>,
    pub id: ZwpPrimarySelectionOfferV1Id,
}
impl EventFormatter for DataOffer {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, DATA_OFFER).object(self.id);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for DataOffer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "data_offer(id: {})", self.id)
    }
}

pub(super) struct Selection {
    pub obj: Rc<ZwpPrimarySelectionDeviceV1>,
    pub id: ZwpPrimarySelectionOfferV1Id,
}
impl EventFormatter for Selection {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, SELECTION).object(self.id);
    }
    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Selection {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "selection(id: {})", self.id)
    }
}
