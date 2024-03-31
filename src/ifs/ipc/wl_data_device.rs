use {
    crate::{
        client::{Client, ClientError, ClientId},
        fixed::Fixed,
        ifs::{
            ipc::{
                break_device_loops, destroy_data_device, wl_data_offer::WlDataOffer,
                wl_data_source::WlDataSource, DeviceData, IpcLocation, IpcVtable,
                IterableIpcVtable, OfferData, Role,
            },
            wl_seat::{WlSeatError, WlSeatGlobal},
            wl_surface::{SurfaceRole, WlSurfaceError},
        },
        leaks::Tracker,
        object::Object,
        utils::buffd::{MsgParser, MsgParserError},
        wire::{wl_data_device::*, WlDataDeviceId, WlDataOfferId, WlSurfaceId},
    },
    std::rc::Rc,
    thiserror::Error,
};

#[allow(dead_code)]
const ROLE: u32 = 0;

pub struct WlDataDevice {
    pub id: WlDataDeviceId,
    pub client: Rc<Client>,
    pub version: u32,
    pub seat: Rc<WlSeatGlobal>,
    pub data: DeviceData<WlDataOffer>,
    pub tracker: Tracker<Self>,
}

impl WlDataDevice {
    pub fn new(
        id: WlDataDeviceId,
        client: &Rc<Client>,
        version: u32,
        seat: &Rc<WlSeatGlobal>,
    ) -> Self {
        Self {
            id,
            client: client.clone(),
            version,
            seat: seat.clone(),
            data: Default::default(),
            tracker: Default::default(),
        }
    }

    pub fn send_data_offer(&self, offer: &Rc<WlDataOffer>) {
        self.client.event(DataOffer {
            self_id: self.id,
            id: offer.id,
        })
    }

    pub fn send_selection(&self, offer: Option<&Rc<WlDataOffer>>) {
        let id = offer.map(|o| o.id).unwrap_or(WlDataOfferId::NONE);
        self.client.event(Selection {
            self_id: self.id,
            id,
        })
    }

    pub fn send_leave(&self) {
        self.client.event(Leave { self_id: self.id })
    }

    pub fn send_enter(
        &self,
        surface: WlSurfaceId,
        x: Fixed,
        y: Fixed,
        offer: WlDataOfferId,
        serial: u32,
    ) {
        self.client.event(Enter {
            self_id: self.id,
            serial,
            surface,
            x,
            y,
            id: offer,
        })
    }

    pub fn send_motion(&self, time_usec: u64, x: Fixed, y: Fixed) {
        self.client.event(Motion {
            self_id: self.id,
            time: (time_usec / 1000) as _,
            x,
            y,
        })
    }

    pub fn send_drop(&self) {
        self.client.event(Drop { self_id: self.id })
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
        self.seat
            .set_wl_data_source_selection(src, Some(req.serial))?;
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

impl IterableIpcVtable for ClipboardIpc {
    fn for_each_device<C>(seat: &WlSeatGlobal, client: ClientId, f: C)
    where
        C: FnMut(&Rc<Self::Device>),
    {
        seat.for_each_data_device(0, client, f);
    }
}

impl IpcVtable for ClipboardIpc {
    const LOCATION: IpcLocation = IpcLocation::Clipboard;

    type Device = WlDataDevice;
    type Source = WlDataSource;
    type Offer = WlDataOffer;

    fn get_device_data(dd: &Self::Device) -> &DeviceData<Self::Offer> {
        &dd.data
    }

    fn get_device_seat(dd: &Self::Device) -> Rc<WlSeatGlobal> {
        dd.seat.clone()
    }

    fn set_seat_selection(
        seat: &Rc<WlSeatGlobal>,
        source: &Rc<Self::Source>,
        serial: Option<u32>,
    ) -> Result<(), WlSeatError> {
        seat.set_wl_data_source_selection(Some(source.clone()), serial)
    }

    fn create_offer(
        device: &Rc<WlDataDevice>,
        offer_data: OfferData<Self::Device>,
    ) -> Result<Rc<Self::Offer>, ClientError> {
        let rc = Rc::new(WlDataOffer {
            id: device.client.new_id()?,
            offer_id: device.client.state.data_offer_ids.next(),
            client: device.client.clone(),
            device: device.clone(),
            data: offer_data,
            tracker: Default::default(),
        });
        track!(device.client, rc);
        device.client.add_server_obj(&rc);
        Ok(rc)
    }

    fn send_selection(dd: &Self::Device, offer: Option<&Rc<Self::Offer>>) {
        dd.send_selection(offer);
    }

    fn send_offer(dd: &Self::Device, offer: &Rc<Self::Offer>) {
        dd.send_data_offer(offer);
    }

    fn unset(seat: &Rc<WlSeatGlobal>, role: Role) {
        match role {
            Role::Selection => seat.unset_selection(),
            Role::Dnd => seat.cancel_dnd(),
        }
    }

    fn device_client(dd: &Rc<Self::Device>) -> &Rc<Client> {
        &dd.client
    }
}

object_base! {
    self = WlDataDevice;

    START_DRAG => start_drag,
    SET_SELECTION => set_selection,
    RELEASE => release if self.version >= 2,
}

impl Object for WlDataDevice {
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
