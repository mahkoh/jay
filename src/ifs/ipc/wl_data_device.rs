use {
    crate::{
        client::{Client, ClientError, ClientId},
        fixed::Fixed,
        ifs::{
            ipc::{
                break_device_loops, destroy_data_device, wl_data_offer::WlDataOffer,
                wl_data_source::WlDataSource, DeviceData, IpcVtable, OfferData, Role, SourceData,
            },
            wl_seat::{WlSeatError, WlSeatGlobal},
            wl_surface::{SurfaceRole, WlSurfaceError},
        },
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{wl_data_device::*, WlDataDeviceId, WlDataOfferId, WlDataSourceId, WlSurfaceId},
        xwayland::XWaylandEvent,
    },
    std::rc::Rc,
    thiserror::Error,
    uapi::OwnedFd,
};

#[allow(dead_code)]
const ROLE: u32 = 0;

pub struct WlDataDevice {
    pub id: WlDataDeviceId,
    pub client: Rc<Client>,
    pub version: u32,
    pub seat: Rc<WlSeatGlobal>,
    pub data: DeviceData<ClipboardIpc>,
    pub tracker: Tracker<Self>,
}

impl WlDataDevice {
    pub fn new(
        id: WlDataDeviceId,
        client: &Rc<Client>,
        version: u32,
        seat: &Rc<WlSeatGlobal>,
        is_xwm: bool,
    ) -> Self {
        Self {
            id,
            client: client.clone(),
            version,
            seat: seat.clone(),
            data: DeviceData {
                selection: Default::default(),
                dnd: Default::default(),
                is_xwm,
            },
            tracker: Default::default(),
        }
    }

    pub fn send_data_offer(&self, offer: &Rc<WlDataOffer>) {
        if self.data.is_xwm {
            self.client
                .state
                .xwayland
                .queue
                .push(XWaylandEvent::ClipboardSetOffer(offer.clone()));
        } else {
            self.client.event(DataOffer {
                self_id: self.id,
                id: offer.id,
            })
        }
    }

    pub fn send_selection(&self, offer: Option<&Rc<WlDataOffer>>) {
        if self.data.is_xwm {
            self.client
                .state
                .xwayland
                .queue
                .push(XWaylandEvent::ClipboardSetSelection(
                    self.seat.id(),
                    offer.cloned(),
                ));
        } else {
            let id = offer.map(|o| o.id).unwrap_or(WlDataOfferId::NONE);
            self.client.event(Selection {
                self_id: self.id,
                id,
            })
        }
    }

    pub fn send_leave(&self) {
        if !self.data.is_xwm {
            self.client.event(Leave { self_id: self.id })
        }
    }

    pub fn send_enter(
        &self,
        surface: WlSurfaceId,
        x: Fixed,
        y: Fixed,
        offer: WlDataOfferId,
        serial: u32,
    ) {
        if !self.data.is_xwm {
            self.client.event(Enter {
                self_id: self.id,
                serial,
                surface,
                x,
                y,
                id: offer,
            })
        }
    }

    pub fn send_motion(&self, time_usec: u64, x: Fixed, y: Fixed) {
        if !self.data.is_xwm {
            self.client.event(Motion {
                self_id: self.id,
                time: (time_usec / 1000) as _,
                x,
                y,
            })
        }
    }

    pub fn send_drop(&self) {
        if !self.data.is_xwm {
            self.client.event(Drop { self_id: self.id })
        }
    }

    fn start_drag(&self, parser: MsgParser<'_, '_>) -> Result<(), WlDataDeviceError> {
        let req: StartDrag = self.client.parse(self, parser)?;
        if !self.client.valid_serial(req.serial) {
            log::warn!("Client tried to start_drag with an invalid serial");
            return Ok(());
        }
        let origin = self.client.lookup(req.origin)?;
        let source = if req.source.is_some() {
            Some(self.client.lookup(req.source)?)
        } else {
            None
        };
        let icon = if req.icon.is_some() {
            let icon = self.client.lookup(req.icon)?;
            icon.set_role(SurfaceRole::DndIcon)?;
            Some(icon)
        } else {
            None
        };
        self.seat.start_drag(&origin, source, icon, req.serial)?;
        Ok(())
    }

    fn set_selection(&self, parser: MsgParser<'_, '_>) -> Result<(), WlDataDeviceError> {
        let req: SetSelection = self.client.parse(self, parser)?;
        if !self.client.valid_serial(req.serial) {
            log::warn!("Client tried to set_selection with an invalid serial");
            return Ok(());
        }
        if !self.seat.may_modify_selection(&self.client, req.serial) {
            log::warn!("Ignoring disallowed set_selection request");
            return Ok(());
        }
        let src = if req.source.is_none() {
            None
        } else {
            Some(self.client.lookup(req.source)?)
        };
        self.seat.set_selection(src, Some(req.serial))?;
        Ok(())
    }

    fn release(&self, parser: MsgParser<'_, '_>) -> Result<(), WlDataDeviceError> {
        let _req: Release = self.client.parse(self, parser)?;
        destroy_data_device::<ClipboardIpc>(self);
        self.seat.remove_data_device(self);
        self.client.remove_obj(self)?;
        Ok(())
    }
}

pub struct ClipboardIpc;

impl IpcVtable for ClipboardIpc {
    type Device = WlDataDevice;
    type Source = WlDataSource;
    type Offer = WlDataOffer;

    fn get_device_data(dd: &Self::Device) -> &DeviceData<Self> {
        &dd.data
    }

    fn get_device_seat(dd: &Self::Device) -> Rc<WlSeatGlobal> {
        dd.seat.clone()
    }

    fn create_xwm_source(client: &Rc<Client>) -> Self::Source {
        WlDataSource::new(WlDataSourceId::NONE, client, true)
    }

    fn set_seat_selection(
        seat: &Rc<WlSeatGlobal>,
        source: &Rc<Self::Source>,
        serial: Option<u32>,
    ) -> Result<(), WlSeatError> {
        seat.set_selection(Some(source.clone()), serial)
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
    ) -> Result<Rc<Self::Offer>, ClientError> {
        let rc = Rc::new(WlDataOffer {
            id: client.new_id()?,
            u64_id: client.state.data_offer_ids.fetch_add(1),
            client: client.clone(),
            device: device.clone(),
            data: offer_data,
            tracker: Default::default(),
        });
        track!(client, rc);
        Ok(rc)
    }

    fn send_selection(dd: &Self::Device, offer: Option<&Rc<Self::Offer>>) {
        dd.send_selection(offer);
    }

    fn send_cancelled(source: &Rc<Self::Source>) {
        source.send_cancelled();
    }

    fn get_offer_id(offer: &Self::Offer) -> u64 {
        offer.u64_id
    }

    fn send_offer(dd: &Self::Device, offer: &Rc<Self::Offer>) {
        dd.send_data_offer(offer);
    }

    fn send_mime_type(offer: &Rc<Self::Offer>, mime_type: &str) {
        offer.send_offer(mime_type);
    }

    fn unset(seat: &Rc<WlSeatGlobal>, role: Role) {
        match role {
            Role::Selection => seat.unset_selection(),
            Role::Dnd => seat.cancel_dnd(),
        }
    }

    fn send_send(src: &Rc<Self::Source>, mime_type: &str, fd: Rc<OwnedFd>) {
        src.send_send(mime_type, fd);
    }

    fn remove_from_seat(device: &Self::Device) {
        device.seat.remove_data_device(device);
    }

    fn get_offer_seat(offer: &Self::Offer) -> Rc<WlSeatGlobal> {
        offer.device.seat.clone()
    }
}

object_base! {
    WlDataDevice;

    START_DRAG => start_drag,
    SET_SELECTION => set_selection,
    RELEASE => release,
}

impl Object for WlDataDevice {
    fn num_requests(&self) -> u32 {
        RELEASE + 1
    }

    fn break_loops(&self) {
        break_device_loops::<ClipboardIpc>(self);
        self.seat.remove_data_device(self);
    }
}

simple_add_obj!(WlDataDevice);

#[derive(Debug, Error)]
pub enum WlDataDeviceError {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WlSeatError(Box<WlSeatError>),
    #[error(transparent)]
    WlSurfaceError(Box<WlSurfaceError>),
}
efrom!(WlDataDeviceError, MsgParserError);
efrom!(WlDataDeviceError, ClientError);
efrom!(WlDataDeviceError, WlSeatError);
efrom!(WlDataDeviceError, WlSurfaceError);
