use crate::client::ClientError;
use crate::ifs::wl_data_device_manager::WlDataDeviceManager;
use crate::ifs::wl_data_source::WlDataSourceError;
use crate::ifs::wl_seat::WlSeat;
use crate::object::Object;
use crate::utils::buffd::MsgParser;
use crate::utils::buffd::MsgParserError;
use crate::wire::wl_data_device::*;
use crate::wire::{WlDataDeviceId, WlDataOfferId};
use std::rc::Rc;
use thiserror::Error;

#[allow(dead_code)]
const ROLE: u32 = 0;

pub struct WlDataDevice {
    pub id: WlDataDeviceId,
    pub manager: Rc<WlDataDeviceManager>,
    seat: Rc<WlSeat>,
}

impl WlDataDevice {
    pub fn new(id: WlDataDeviceId, manager: &Rc<WlDataDeviceManager>, seat: &Rc<WlSeat>) -> Self {
        Self {
            id,
            manager: manager.clone(),
            seat: seat.clone(),
        }
    }

    pub fn send_data_offer(&self, id: WlDataOfferId) {
        self.manager.client.event(DataOffer {
            self_id: self.id,
            id,
        })
    }

    pub fn send_selection(&self, id: WlDataOfferId) {
        self.manager.client.event(Selection {
            self_id: self.id,
            id,
        })
    }

    fn start_drag(&self, parser: MsgParser<'_, '_>) -> Result<(), StartDragError> {
        let _req: StartDrag = self.manager.client.parse(self, parser)?;
        Ok(())
    }

    fn set_selection(&self, parser: MsgParser<'_, '_>) -> Result<(), SetSelectionError> {
        let req: SetSelection = self.manager.client.parse(self, parser)?;
        let src = if req.source.is_none() {
            None
        } else {
            Some(self.manager.client.lookup(req.source)?)
        };
        self.seat.global.set_selection(src)?;
        Ok(())
    }

    fn release(&self, parser: MsgParser<'_, '_>) -> Result<(), ReleaseError> {
        let _req: Release = self.manager.client.parse(self, parser)?;
        self.seat.remove_data_device(self);
        self.manager.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    WlDataDevice, WlDataDeviceError;

    START_DRAG => start_drag,
    SET_SELECTION => set_selection,
    RELEASE => release,
}

impl Object for WlDataDevice {
    fn num_requests(&self) -> u32 {
        RELEASE + 1
    }

    fn break_loops(&self) {
        self.seat.remove_data_device(self);
    }
}

simple_add_obj!(WlDataDevice);

#[derive(Debug, Error)]
pub enum WlDataDeviceError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process `start_drag` request")]
    StartDragError(#[from] StartDragError),
    #[error("Could not process `set_selection` request")]
    SetSelectionError(#[from] SetSelectionError),
    #[error("Could not process `release` request")]
    ReleaseError(#[from] ReleaseError),
}
efrom!(WlDataDeviceError, ClientError);

#[derive(Debug, Error)]
pub enum StartDragError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(StartDragError, ParseFailed, MsgParserError);
efrom!(StartDragError, ClientError);

#[derive(Debug, Error)]
pub enum SetSelectionError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WlDataSourceError(Box<WlDataSourceError>),
}
efrom!(SetSelectionError, ParseFailed, MsgParserError);
efrom!(SetSelectionError, ClientError);
efrom!(SetSelectionError, WlDataSourceError);

#[derive(Debug, Error)]
pub enum ReleaseError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ReleaseError, ParseFailed, MsgParserError);
efrom!(ReleaseError, ClientError);
