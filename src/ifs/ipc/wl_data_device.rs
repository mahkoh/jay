use {
    crate::{
        client::{Client, ClientError, ClientId},
        fixed::Fixed,
        ifs::{
            ipc::{
                DeviceData, IpcVtable, IterableIpcVtable, OfferData, Role, break_device_loops,
                destroy_data_device, wl_data_offer::WlDataOffer, wl_data_source::WlDataSource,
            },
            wl_seat::{WlSeatError, WlSeatGlobal},
            wl_surface::WlSurfaceError,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{WlDataDeviceId, WlDataOfferId, WlSurfaceId, wl_data_device::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

#[expect(dead_code)]
const ROLE: u32 = 0;

pub struct WlDataDevice {
    pub id: WlDataDeviceId,
    pub client: Rc<Client>,
    pub version: Version,
    pub seat: Rc<WlSeatGlobal>,
    pub data: DeviceData<WlDataOffer>,
    pub tracker: Tracker<Self>,
}

impl WlDataDevice {
    pub fn new(
        id: WlDataDeviceId,
        client: &Rc<Client>,
        version: Version,
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
        serial: u64,
    ) {
        self.client.event(Enter {
            self_id: self.id,
            serial: serial as _,
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
}

impl WlDataDeviceRequestHandler for WlDataDevice {
    type Error = WlDataDeviceError;

    fn start_drag(&self, req: StartDrag, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(serial) = self.client.map_serial(req.serial) else {
            log::warn!("Client tried to start_drag with an invalid serial");
            return Ok(());
        };
        let origin = self.client.lookup(req.origin)?;
        let source = if req.source.is_some() {
            Some(self.client.lookup(req.source)?)
        } else {
            None
        };
        let icon = if req.icon.is_some() {
            let icon = self.client.lookup(req.icon)?;
            Some(icon.into_dnd_icon(&self.seat)?)
        } else {
            None
        };
        self.seat.start_drag(&origin, source, icon, serial)?;
        Ok(())
    }

    fn set_selection(&self, req: SetSelection, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(serial) = self.client.map_serial(req.serial) else {
            log::warn!("Client tried to set_selection with an invalid serial");
            return Ok(());
        };
        if !self.seat.may_modify_selection(&self.client, serial) {
            log::warn!("Ignoring disallowed set_selection request");
            return Ok(());
        }
        let src = if req.source.is_none() {
            None
        } else {
            Some(self.client.lookup(req.source)?)
        };
        self.seat.set_wl_data_source_selection(src, Some(serial))?;
        Ok(())
    }

    fn release(&self, _req: Release, _slf: &Rc<Self>) -> Result<(), Self::Error> {
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
        seat.for_each_data_device(Version::ALL, client, f);
    }
}

impl IpcVtable for ClipboardIpc {
    type Device = WlDataDevice;
    type Source = WlDataSource;
    type Offer = WlDataOffer;

    fn get_device_data(dd: &Self::Device) -> &DeviceData<Self::Offer> {
        &dd.data
    }

    fn get_device_seat(dd: &Self::Device) -> Rc<WlSeatGlobal> {
        dd.seat.clone()
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
    version = self.version;
}

impl Object for WlDataDevice {
    fn break_loops(self: Rc<Self>) {
        break_device_loops::<ClipboardIpc>(&*self);
        self.seat.remove_data_device(&*self);
    }
}

simple_add_obj!(WlDataDevice);

#[derive(Debug, Error)]
pub enum WlDataDeviceError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WlSeatError(Box<WlSeatError>),
    #[error(transparent)]
    WlSurfaceError(Box<WlSurfaceError>),
}
efrom!(WlDataDeviceError, ClientError);
efrom!(WlDataDeviceError, WlSeatError);
efrom!(WlDataDeviceError, WlSurfaceError);
