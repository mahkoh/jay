use {
    crate::{
        client::{Client, ClientError, ClientId},
        ifs::{
            ipc::{
                break_device_loops, destroy_data_device,
                zwp_primary_selection_offer_v1::ZwpPrimarySelectionOfferV1,
                zwp_primary_selection_source_v1::ZwpPrimarySelectionSourceV1, DeviceData,
                IpcVtable, IterableIpcVtable, OfferData, Role,
            },
            wl_seat::{WlSeatError, WlSeatGlobal},
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{
            zwp_primary_selection_device_v1::*, ZwpPrimarySelectionDeviceV1Id,
            ZwpPrimarySelectionOfferV1Id,
        },
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwpPrimarySelectionDeviceV1 {
    pub id: ZwpPrimarySelectionDeviceV1Id,
    pub client: Rc<Client>,
    pub version: Version,
    pub seat: Rc<WlSeatGlobal>,
    data: DeviceData<ZwpPrimarySelectionOfferV1>,
    pub tracker: Tracker<Self>,
}

impl ZwpPrimarySelectionDeviceV1 {
    pub fn new(
        id: ZwpPrimarySelectionDeviceV1Id,
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

    pub fn send_data_offer(&self, offer: &Rc<ZwpPrimarySelectionOfferV1>) {
        self.client.event(DataOffer {
            self_id: self.id,
            offer: offer.id,
        })
    }

    pub fn send_selection(&self, offer: Option<&Rc<ZwpPrimarySelectionOfferV1>>) {
        let id = offer
            .map(|o| o.id)
            .unwrap_or(ZwpPrimarySelectionOfferV1Id::NONE);
        self.client.event(Selection {
            self_id: self.id,
            id,
        })
    }
}

impl ZwpPrimarySelectionDeviceV1RequestHandler for ZwpPrimarySelectionDeviceV1 {
    type Error = ZwpPrimarySelectionDeviceV1Error;

    fn set_selection(&self, req: SetSelection, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let Some(serial) = self.client.map_serial(req.serial) else {
            log::warn!("Client tried to set_selection with an invalid serial");
            return Ok(());
        };
        if !self
            .seat
            .may_modify_primary_selection(&self.client, Some(serial))
        {
            log::warn!("Ignoring disallowed set_selection request");
            return Ok(());
        }
        let src = if req.source.is_none() {
            None
        } else {
            Some(self.client.lookup(req.source)?)
        };
        self.seat.set_zwp_primary_selection(src, Some(serial))?;
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        destroy_data_device::<PrimarySelectionIpc>(self);
        self.seat.remove_primary_selection_device(self);
        self.client.remove_obj(self)?;
        Ok(())
    }
}

pub struct PrimarySelectionIpc;

impl IterableIpcVtable for PrimarySelectionIpc {
    fn for_each_device<C>(seat: &WlSeatGlobal, client: ClientId, f: C)
    where
        C: FnMut(&Rc<Self::Device>),
    {
        seat.for_each_primary_selection_device(Version::ALL, client, f)
    }
}

impl IpcVtable for PrimarySelectionIpc {
    type Device = ZwpPrimarySelectionDeviceV1;
    type Source = ZwpPrimarySelectionSourceV1;
    type Offer = ZwpPrimarySelectionOfferV1;

    fn get_device_data(dd: &Self::Device) -> &DeviceData<Self::Offer> {
        &dd.data
    }

    fn get_device_seat(dd: &Self::Device) -> Rc<WlSeatGlobal> {
        dd.seat.clone()
    }

    fn create_offer(
        device: &Rc<ZwpPrimarySelectionDeviceV1>,
        offer_data: OfferData<Self::Device>,
    ) -> Result<Rc<Self::Offer>, ClientError> {
        let rc = Rc::new(ZwpPrimarySelectionOfferV1 {
            id: device.client.new_id()?,
            offer_id: device.client.state.data_offer_ids.next(),
            seat: device.seat.clone(),
            client: device.client.clone(),
            data: offer_data,
            tracker: Default::default(),
            version: device.version,
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

    fn unset(seat: &Rc<WlSeatGlobal>, _role: Role) {
        seat.unset_primary_selection();
    }

    fn device_client(dd: &Rc<Self::Device>) -> &Rc<Client> {
        &dd.client
    }
}

object_base! {
    self = ZwpPrimarySelectionDeviceV1;
    version = self.version;
}

impl Object for ZwpPrimarySelectionDeviceV1 {
    fn break_loops(&self) {
        break_device_loops::<PrimarySelectionIpc>(self);
        self.seat.remove_primary_selection_device(self);
    }
}

simple_add_obj!(ZwpPrimarySelectionDeviceV1);

#[derive(Debug, Error)]
pub enum ZwpPrimarySelectionDeviceV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WlSeatError(Box<WlSeatError>),
}
efrom!(ZwpPrimarySelectionDeviceV1Error, ClientError);
efrom!(ZwpPrimarySelectionDeviceV1Error, WlSeatError);
