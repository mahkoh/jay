use {
    crate::{
        client::{Client, ClientError},
        ifs::{
            ipc::{
                break_device_loops, destroy_data_device,
                zwlr_data_control_device_v1::private::{
                    WlrClipboardIpcCore, WlrIpcImpl, WlrPrimarySelectionIpcCore,
                },
                zwlr_data_control_offer_v1::ZwlrDataControlOfferV1,
                zwlr_data_control_source_v1::ZwlrDataControlSourceV1,
                DeviceData, IpcLocation, IpcVtable, OfferData, Role, WlrIpcVtable,
            },
            wl_seat::{WlSeatError, WlSeatGlobal},
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{
            zwlr_data_control_device_v1::*, ZwlrDataControlDeviceV1Id, ZwlrDataControlOfferV1Id,
            ZwlrDataControlSourceV1Id,
        },
    },
    std::rc::Rc,
    thiserror::Error,
};

pub const PRIMARY_SELECTION_SINCE: Version = Version(2);

pub struct ZwlrDataControlDeviceV1 {
    pub id: ZwlrDataControlDeviceV1Id,
    pub client: Rc<Client>,
    pub version: Version,
    pub seat: Rc<WlSeatGlobal>,
    pub clipboard_data: DeviceData<ZwlrDataControlOfferV1>,
    pub primary_selection_data: DeviceData<ZwlrDataControlOfferV1>,
    pub tracker: Tracker<Self>,
}

impl ZwlrDataControlDeviceV1 {
    pub fn new(
        id: ZwlrDataControlDeviceV1Id,
        client: &Rc<Client>,
        version: Version,
        seat: &Rc<WlSeatGlobal>,
    ) -> Self {
        Self {
            id,
            client: client.clone(),
            version,
            seat: seat.clone(),
            clipboard_data: Default::default(),
            primary_selection_data: Default::default(),
            tracker: Default::default(),
        }
    }

    pub fn send_data_offer(&self, offer: &Rc<ZwlrDataControlOfferV1>) {
        self.client.event(DataOffer {
            self_id: self.id,
            id: offer.id,
        })
    }

    pub fn send_selection(&self, offer: Option<&Rc<ZwlrDataControlOfferV1>>) {
        let id = offer
            .map(|o| o.id)
            .unwrap_or(ZwlrDataControlOfferV1Id::NONE);
        self.client.event(Selection {
            self_id: self.id,
            id,
        })
    }

    pub fn send_primary_selection(&self, offer: Option<&Rc<ZwlrDataControlOfferV1>>) {
        let id = offer
            .map(|o| o.id)
            .unwrap_or(ZwlrDataControlOfferV1Id::NONE);
        self.client.event(PrimarySelection {
            self_id: self.id,
            id,
        })
    }

    fn use_source(
        &self,
        source: ZwlrDataControlSourceV1Id,
        location: IpcLocation,
    ) -> Result<Option<Rc<ZwlrDataControlSourceV1>>, ZwlrDataControlDeviceV1Error> {
        if source.is_none() {
            Ok(None)
        } else {
            let src = self.client.lookup(source)?;
            if src.used.replace(true) {
                return Err(ZwlrDataControlDeviceV1Error::AlreadyUsed);
            }
            src.location.set(location);
            Ok(Some(src))
        }
    }
}

impl ZwlrDataControlDeviceV1RequestHandler for ZwlrDataControlDeviceV1 {
    type Error = ZwlrDataControlDeviceV1Error;

    fn set_selection(&self, req: SetSelection, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let src = self.use_source(req.source, IpcLocation::Clipboard)?;
        self.seat.set_selection(src)?;
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        destroy_data_device::<WlrClipboardIpc>(self);
        destroy_data_device::<WlrPrimarySelectionIpc>(self);
        self.seat.remove_wlr_device(self);
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_primary_selection(
        &self,
        req: SetPrimarySelection,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let src = self.use_source(req.source, IpcLocation::PrimarySelection)?;
        self.seat.set_primary_selection(src)?;
        Ok(())
    }
}

mod private {
    use std::marker::PhantomData;

    pub struct WlrClipboardIpcCore;
    pub struct WlrPrimarySelectionIpcCore;
    pub struct WlrIpcImpl<T>(PhantomData<T>);
}
pub type WlrClipboardIpc = WlrIpcImpl<WlrClipboardIpcCore>;
pub type WlrPrimarySelectionIpc = WlrIpcImpl<WlrPrimarySelectionIpcCore>;

trait WlrIpc {
    const MIN_VERSION: Version;
    const LOCATION: IpcLocation;

    fn wlr_get_device_data(dd: &ZwlrDataControlDeviceV1) -> &DeviceData<ZwlrDataControlOfferV1>;

    fn wlr_set_seat_selection(
        seat: &Rc<WlSeatGlobal>,
        source: &Rc<ZwlrDataControlSourceV1>,
    ) -> Result<(), WlSeatError>;

    fn wlr_send_selection(dd: &ZwlrDataControlDeviceV1, offer: Option<&Rc<ZwlrDataControlOfferV1>>);

    fn wlr_unset(seat: &Rc<WlSeatGlobal>);
}

impl WlrIpc for WlrClipboardIpcCore {
    const MIN_VERSION: Version = Version::ALL;
    const LOCATION: IpcLocation = IpcLocation::Clipboard;

    fn wlr_get_device_data(dd: &ZwlrDataControlDeviceV1) -> &DeviceData<ZwlrDataControlOfferV1> {
        &dd.clipboard_data
    }

    fn wlr_set_seat_selection(
        seat: &Rc<WlSeatGlobal>,
        source: &Rc<ZwlrDataControlSourceV1>,
    ) -> Result<(), WlSeatError> {
        seat.set_selection(Some(source.clone()))
    }

    fn wlr_send_selection(
        dd: &ZwlrDataControlDeviceV1,
        offer: Option<&Rc<ZwlrDataControlOfferV1>>,
    ) {
        dd.send_selection(offer)
    }

    fn wlr_unset(seat: &Rc<WlSeatGlobal>) {
        seat.unset_selection()
    }
}

impl WlrIpc for WlrPrimarySelectionIpcCore {
    const MIN_VERSION: Version = PRIMARY_SELECTION_SINCE;
    const LOCATION: IpcLocation = IpcLocation::PrimarySelection;

    fn wlr_get_device_data(dd: &ZwlrDataControlDeviceV1) -> &DeviceData<ZwlrDataControlOfferV1> {
        &dd.primary_selection_data
    }

    fn wlr_set_seat_selection(
        seat: &Rc<WlSeatGlobal>,
        source: &Rc<ZwlrDataControlSourceV1>,
    ) -> Result<(), WlSeatError> {
        seat.set_primary_selection(Some(source.clone()))
    }

    fn wlr_send_selection(
        dd: &ZwlrDataControlDeviceV1,
        offer: Option<&Rc<ZwlrDataControlOfferV1>>,
    ) {
        dd.send_primary_selection(offer)
    }

    fn wlr_unset(seat: &Rc<WlSeatGlobal>) {
        seat.unset_primary_selection()
    }
}

impl<T: WlrIpc> WlrIpcVtable for WlrIpcImpl<T> {
    fn for_each_device<C>(seat: &WlSeatGlobal, f: C)
    where
        C: FnMut(&Rc<Self::Device>),
    {
        seat.for_each_wlr_data_device(T::MIN_VERSION, f)
    }
}

impl<T: WlrIpc> IpcVtable for WlrIpcImpl<T> {
    type Device = ZwlrDataControlDeviceV1;
    type Source = ZwlrDataControlSourceV1;
    type Offer = ZwlrDataControlOfferV1;

    fn get_device_data(dd: &Self::Device) -> &DeviceData<Self::Offer> {
        T::wlr_get_device_data(dd)
    }

    fn get_device_seat(dd: &Self::Device) -> Rc<WlSeatGlobal> {
        dd.seat.clone()
    }

    fn set_seat_selection(
        seat: &Rc<WlSeatGlobal>,
        source: &Rc<Self::Source>,
        serial: Option<u32>,
    ) -> Result<(), WlSeatError> {
        debug_assert!(serial.is_none());
        let _ = serial;
        T::wlr_set_seat_selection(seat, source)
    }

    fn create_offer(
        device: &Rc<ZwlrDataControlDeviceV1>,
        offer_data: OfferData<Self::Device>,
    ) -> Result<Rc<Self::Offer>, ClientError> {
        let rc = Rc::new(ZwlrDataControlOfferV1 {
            id: device.client.new_id()?,
            offer_id: device.client.state.data_offer_ids.next(),
            client: device.client.clone(),
            device: device.clone(),
            data: offer_data,
            location: T::LOCATION,
            tracker: Default::default(),
        });
        track!(device.client, rc);
        device.client.add_server_obj(&rc);
        Ok(rc)
    }

    fn send_selection(dd: &Self::Device, offer: Option<&Rc<Self::Offer>>) {
        T::wlr_send_selection(dd, offer)
    }

    fn send_offer(dd: &Self::Device, offer: &Rc<Self::Offer>) {
        dd.send_data_offer(offer);
    }

    fn unset(seat: &Rc<WlSeatGlobal>, _role: Role) {
        T::wlr_unset(seat)
    }

    fn device_client(dd: &Rc<Self::Device>) -> &Rc<Client> {
        &dd.client
    }
}

object_base! {
    self = ZwlrDataControlDeviceV1;
    version = self.version;
}

impl Object for ZwlrDataControlDeviceV1 {
    fn break_loops(&self) {
        break_device_loops::<WlrClipboardIpc>(self);
        break_device_loops::<WlrPrimarySelectionIpc>(self);
        self.seat.remove_wlr_device(self);
    }
}

simple_add_obj!(ZwlrDataControlDeviceV1);

#[derive(Debug, Error)]
pub enum ZwlrDataControlDeviceV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    WlSeatError(Box<WlSeatError>),
    #[error("The source has already been used")]
    AlreadyUsed,
}
efrom!(ZwlrDataControlDeviceV1Error, ClientError);
efrom!(ZwlrDataControlDeviceV1Error, WlSeatError);
