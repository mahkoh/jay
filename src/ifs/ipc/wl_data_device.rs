use crate::client::{Client, ClientError, ClientId};
use crate::ifs::ipc::wl_data_device_manager::WlDataDeviceManager;
use crate::ifs::ipc::wl_data_source::{WlDataSource};
use crate::ifs::wl_seat::{WlSeat, WlSeatError, WlSeatGlobal};
use crate::object::{Object, ObjectId};
use crate::utils::buffd::MsgParser;
use crate::utils::buffd::MsgParserError;
use crate::wire::wl_data_device::*;
use crate::wire::{WlDataDeviceId, WlDataOfferId};
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;
use crate::ifs::ipc::{OfferData, SourceData, Vtable};
use crate::ifs::ipc::wl_data_offer::WlDataOffer;

#[allow(dead_code)]
const ROLE: u32 = 0;

pub struct WlDataDevice {
    pub id: WlDataDeviceId,
    pub manager: Rc<WlDataDeviceManager>,
    pub seat: Rc<WlSeat>,
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

impl Vtable for WlDataDevice {
    type DeviceId = WlDataDeviceId;
    type OfferId = WlDataOfferId;
    type Device = WlDataDevice;
    type Source = WlDataSource;
    type Offer = WlDataOffer;

    fn device_id(dd: &Self::Device) -> Self::DeviceId {
        dd.id
    }

    fn get_offer_data(offer: &Self::Offer) -> &OfferData<Self> {
        &offer.offer_data
    }

    fn get_source_data(src: &Self::Source) -> &SourceData<Self> {
        &src.data
    }

    fn for_each_device<C>(seat: &WlSeatGlobal, client: ClientId, f: C) where C: FnMut(&Rc<Self::Device>) {
        seat.for_each_data_device(0, client, f);
    }

    fn create_offer(client: &Rc<Client>, offer_data: OfferData<Self>, id: ObjectId) -> Self::Offer {
        WlDataOffer {
            id: id.into(),
            client: client.clone(),
            offer_data,
        }
    }

    fn send_selection(dd: &Self::Device, offer: Self::OfferId) {
        dd.send_selection(offer);
    }

    fn send_cancelled(source: &Self::Source) {
        source.send_cancelled();
    }

    fn get_offer_id(offer: &Self::Offer) -> Self::OfferId {
        offer.id
    }

    fn send_offer(dd: &Self::Device, offer: &Self::Offer) {
        dd.send_data_offer(offer.id);
    }

    fn send_mime_type(offer: &Self::Offer, mime_type: &str) {
        offer.send_offer(mime_type);
    }

    fn unset(seat: &Rc<WlSeatGlobal>) {
        seat.unset_selection();
    }

    fn send_send(src: &Self::Source, mime_type: &str, fd: Rc<OwnedFd>) {
        src.send_send(mime_type, fd);
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
    WlSeatError(Box<WlSeatError>),
}
efrom!(SetSelectionError, ParseFailed, MsgParserError);
efrom!(SetSelectionError, ClientError);
efrom!(SetSelectionError, WlSeatError);

#[derive(Debug, Error)]
pub enum ReleaseError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ReleaseError, ParseFailed, MsgParserError);
efrom!(ReleaseError, ClientError);
