
use crate::client::{ClientError, DynEventFormatter};
use crate::ifs::wl_seat::WlSeat;
use crate::ifs::zwp_primary_selection_device_manager_v1::ZwpPrimarySelectionDeviceManagerV1;
use crate::object::Object;
use crate::utils::buffd::{MsgParser, MsgParserError};
use std::rc::Rc;
use thiserror::Error;
use crate::ifs::zwp_primary_selection_source_v1::ZwpPrimarySelectionSourceV1Error;
use crate::wire::zwp_primary_selection_device_v1::*;
use crate::wire::{ZwpPrimarySelectionDeviceV1Id, ZwpPrimarySelectionOfferV1Id};

pub struct ZwpPrimarySelectionDeviceV1 {
    pub id: ZwpPrimarySelectionDeviceV1Id,
    pub manager: Rc<ZwpPrimarySelectionDeviceManagerV1>,
    seat: Rc<WlSeat>,
}

impl ZwpPrimarySelectionDeviceV1 {
    pub fn new(
        id: ZwpPrimarySelectionDeviceV1Id,
        manager: &Rc<ZwpPrimarySelectionDeviceManagerV1>,
        seat: &Rc<WlSeat>,
    ) -> Self {
        Self {
            id,
            manager: manager.clone(),
            seat: seat.clone(),
        }
    }

    pub fn data_offer(self: &Rc<Self>, offer: ZwpPrimarySelectionOfferV1Id) -> DynEventFormatter {
        Box::new(DataOffer {
            self_id: self.id,
            offer,
        })
    }

    pub fn selection(self: &Rc<Self>, id: ZwpPrimarySelectionOfferV1Id) -> DynEventFormatter {
        Box::new(Selection {
            self_id: self.id,
            id,
        })
    }

    fn set_selection(&self, parser: MsgParser<'_, '_>) -> Result<(), SetSelectionError> {
        let req: SetSelection = self.manager.client.parse(self, parser)?;
        let src = if req.source.is_none() {
            None
        } else {
            Some(self.manager.client.lookup(req.source)?)
        };
        self.seat.global.set_primary_selection(src)?;
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.manager.client.parse(self, parser)?;
        self.seat.remove_primary_selection_device(self);
        self.manager.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    ZwpPrimarySelectionDeviceV1, ZwpPrimarySelectionDeviceV1Error;

    SET_SELECTION => set_selection,
    DESTROY => destroy,
}

impl Object for ZwpPrimarySelectionDeviceV1 {
    fn num_requests(&self) -> u32 {
        DESTROY + 1
    }

    fn break_loops(&self) {
        self.seat.remove_primary_selection_device(self);
    }
}

simple_add_obj!(ZwpPrimarySelectionDeviceV1);

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
