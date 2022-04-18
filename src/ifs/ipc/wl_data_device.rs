use {
    crate::{
        client::{Client, ClientError, ClientId},
        fixed::Fixed,
        ifs::{
            ipc::{
                break_device_loops, destroy_device, wl_data_device_manager::WlDataDeviceManager,
                wl_data_offer::WlDataOffer, wl_data_source::WlDataSource, DeviceData, OfferData,
                Role, SourceData, Vtable,
            },
            wl_seat::{WlSeat, WlSeatError, WlSeatGlobal},
            wl_surface::{SurfaceRole, WlSurfaceError},
        },
        leaks::Tracker,
        object::{Object, ObjectId},
        utils::buffd::{MsgParser, MsgParserError},
        wire::{wl_data_device::*, WlDataDeviceId, WlDataOfferId, WlSurfaceId},
    },
    std::rc::Rc,
    thiserror::Error,
    uapi::OwnedFd,
};

#[allow(dead_code)]
const ROLE: u32 = 0;

pub struct WlDataDevice {
    pub id: WlDataDeviceId,
    pub manager: Rc<WlDataDeviceManager>,
    pub seat: Rc<WlSeat>,
    pub data: DeviceData<WlDataDevice>,
    pub tracker: Tracker<Self>,
}

impl WlDataDevice {
    pub fn new(id: WlDataDeviceId, manager: &Rc<WlDataDeviceManager>, seat: &Rc<WlSeat>) -> Self {
        Self {
            id,
            manager: manager.clone(),
            seat: seat.clone(),
            data: Default::default(),
            tracker: Default::default(),
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

    pub fn send_leave(&self) {
        self.manager.client.event(Leave { self_id: self.id })
    }

    pub fn send_enter(
        &self,
        surface: WlSurfaceId,
        x: Fixed,
        y: Fixed,
        offer: WlDataOfferId,
        serial: u32,
    ) {
        self.manager.client.event(Enter {
            self_id: self.id,
            serial,
            surface,
            x,
            y,
            id: offer,
        })
    }

    pub fn send_motion(&self, x: Fixed, y: Fixed) {
        self.manager.client.event(Motion {
            self_id: self.id,
            time: 0,
            x,
            y,
        })
    }

    pub fn send_drop(&self) {
        self.manager.client.event(Drop { self_id: self.id })
    }

    fn start_drag(&self, parser: MsgParser<'_, '_>) -> Result<(), StartDragError> {
        let req: StartDrag = self.manager.client.parse(self, parser)?;
        if !self.manager.client.valid_serial(req.serial) {
            log::warn!("Client tried to start_drag with an invalid serial");
            return Ok(());
        }
        let origin = self.manager.client.lookup(req.origin)?;
        let source = if req.source.is_some() {
            Some(self.manager.client.lookup(req.source)?)
        } else {
            None
        };
        let icon = if req.icon.is_some() {
            let icon = self.manager.client.lookup(req.icon)?;
            icon.set_role(SurfaceRole::DndIcon)?;
            Some(icon)
        } else {
            None
        };
        self.seat
            .global
            .start_drag(&origin, source, icon, req.serial)?;
        Ok(())
    }

    fn set_selection(&self, parser: MsgParser<'_, '_>) -> Result<(), SetSelectionError> {
        let req: SetSelection = self.manager.client.parse(self, parser)?;
        if !self.manager.client.valid_serial(req.serial) {
            log::warn!("Client tried to set_selection with an invalid serial");
            return Ok(());
        }
        if !self
            .seat
            .global
            .may_modify_selection(&self.seat.client, req.serial)
        {
            log::warn!("Ignoring disallowed set_selection request");
            return Ok(());
        }
        let src = if req.source.is_none() {
            None
        } else {
            Some(self.manager.client.lookup(req.source)?)
        };
        self.seat.global.set_selection(src, Some(req.serial))?;
        Ok(())
    }

    fn release(&self, parser: MsgParser<'_, '_>) -> Result<(), ReleaseError> {
        let _req: Release = self.manager.client.parse(self, parser)?;
        destroy_device::<Self>(self);
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

    fn get_device_data(dd: &Self::Device) -> &DeviceData<Self> {
        &dd.data
    }

    fn get_offer_data(offer: &Self::Offer) -> &OfferData<Self> {
        &offer.data
    }

    fn get_source_data(src: &Self::Source) -> &SourceData<Self> {
        &src.data
    }

    fn for_each_device<C>(seat: &WlSeatGlobal, client: ClientId, f: C)
    where
        C: FnMut(&Rc<Self::Device>),
    {
        seat.for_each_data_device(0, client, f);
    }

    fn create_offer(
        client: &Rc<Client>,
        device: &Rc<WlDataDevice>,
        offer_data: OfferData<Self>,
        id: ObjectId,
    ) -> Rc<Self::Offer> {
        let rc = Rc::new(WlDataOffer {
            id: id.into(),
            client: client.clone(),
            device: device.clone(),
            data: offer_data,
            tracker: Default::default(),
        });
        track!(client, rc);
        rc
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

    fn unset(seat: &Rc<WlSeatGlobal>, role: Role) {
        match role {
            Role::Selection => seat.unset_selection(),
            Role::Dnd => seat.cancel_dnd(),
        }
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
        break_device_loops::<Self>(self);
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
    WlSurfaceError(Box<WlSurfaceError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WlSeatError(Box<WlSeatError>),
}
efrom!(StartDragError, ParseFailed, MsgParserError);
efrom!(StartDragError, ClientError);
efrom!(StartDragError, WlSeatError);
efrom!(StartDragError, WlSurfaceError);

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
