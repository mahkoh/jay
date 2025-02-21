use {
    crate::{
        client::{Client, ClientError, ClientId, WaylandObject, WaylandObjectLookup},
        ifs::{
            ipc::{
                DataOffer, DataOfferId, DataSource, DeviceData, DynDataOffer, DynDataSource,
                IpcLocation, IpcVtable, OfferData, Role, SourceData, cancel_offer, cancel_offers,
                data_control::{DataControlDeviceId, DynDataControlDevice},
                detach_seat, offer_source_to_data_control_device, offer_source_to_x,
                x_data_device::{XClipboardIpc, XIpcDevice, XPrimarySelectionIpc},
            },
            wl_seat::WlSeatGlobal,
        },
        object::{ObjectId, Version},
    },
    std::{cell::Cell, marker::PhantomData, rc::Rc},
    uapi::OwnedFd,
};

struct ClipboardCore<T>(PhantomData<T>);
struct PrimarySelectionCore<T>(PhantomData<T>);
struct DataControlIpcImpl<T>(PhantomData<T>);

type Device<T> = <T as DataControlIpc>::Device;
type Offer<T> = <T as DataControlIpc>::Offer;
type Source<T> = <T as DataControlIpc>::Source;
type SourceId<T> = <T as DataControlIpc>::SourceId;

pub trait DataControlIpc: Sized + 'static {
    const PRIMARY_SELECTION_SINCE: Version;

    type Device: DataControlDevice<Ipc = Self>;
    type OfferId: From<ObjectId>;
    type Offer: DataControlOffer<Ipc = Self>;
    type SourceId: WaylandObjectLookup<Object = Self::Source>;
    type Source: DataControlSource<Ipc = Self>;

    fn create_offer(id: Self::OfferId, data: DataControlOfferData<Self>) -> Rc<Self::Offer>;
}

pub struct DataControlDeviceData<T: DataControlIpc> {
    pub data_control_device_id: DataControlDeviceId,
    pub client: Rc<Client>,
    pub version: Version,
    pub seat: Rc<WlSeatGlobal>,
    pub clipboard_data: DeviceData<T::Offer>,
    pub primary_selection_data: DeviceData<T::Offer>,
}

pub trait DataControlDevice: WaylandObject {
    type Ipc: DataControlIpc<Device = Self>;

    fn data(&self) -> &DataControlDeviceData<Self::Ipc>;

    fn send_data_offer(&self, offer: &Rc<Offer<Self::Ipc>>);

    fn send_selection(&self, offer: Option<&Rc<Offer<Self::Ipc>>>);

    fn send_primary_selection(&self, offer: Option<&Rc<Offer<Self::Ipc>>>);
}

pub struct DataControlOfferData<T: DataControlIpc> {
    pub offer_id: DataOfferId,
    pub client: Rc<Client>,
    pub device: Rc<T::Device>,
    pub data: OfferData<T::Device>,
    pub location: IpcLocation,
}

pub trait DataControlOffer: WaylandObject {
    type Ipc: DataControlIpc<Offer = Self>;

    fn data(&self) -> &DataControlOfferData<Self::Ipc>;

    fn send_offer(&self, mime_type: &str);
}

pub struct DataControlSourceData {
    pub data: SourceData,
    pub version: Version,
    pub location: Cell<IpcLocation>,
    pub used: Cell<bool>,
}

pub trait DataControlSource: WaylandObject {
    type Ipc: DataControlIpc<Source = Self>;

    fn data(&self) -> &DataControlSourceData;

    fn send_cancelled(&self);

    fn send_send(&self, mime_type: &str, fd: Rc<OwnedFd>);
}

impl<T: DataControlDevice> DynDataControlDevice for T {
    fn id(&self) -> DataControlDeviceId {
        self.data().data_control_device_id
    }

    fn handle_new_source(
        self: Rc<Self>,
        location: IpcLocation,
        source: Option<Rc<dyn DynDataSource>>,
    ) {
        if location == IpcLocation::PrimarySelection
            && self.data().version < T::Ipc::PRIMARY_SELECTION_SINCE
        {
            return;
        }
        match location {
            IpcLocation::Clipboard => match source {
                Some(src) => {
                    offer_source_to_data_control_device::<Clipboard<T::Ipc>>(src, &self);
                }
                _ => self.send_selection(None),
            },
            IpcLocation::PrimarySelection => match source {
                Some(src) => {
                    offer_source_to_data_control_device::<PrimarySelection<T::Ipc>>(src, &self);
                }
                _ => self.send_primary_selection(None),
            },
        }
    }
}

type Clipboard<T> = DataControlIpcImpl<ClipboardCore<T>>;
type PrimarySelection<T> = DataControlIpcImpl<PrimarySelectionCore<T>>;

pub trait DataControlLocationIpc {
    type Ipc: DataControlIpc;
    const LOCATION: IpcLocation;

    fn loc_get_device_data(dd: &Device<Self::Ipc>) -> &DeviceData<Offer<Self::Ipc>>;

    fn loc_send_selection(dd: &Device<Self::Ipc>, offer: Option<&Rc<Offer<Self::Ipc>>>);

    fn loc_unset(seat: &Rc<WlSeatGlobal>);
}

impl<T: DataControlIpc> DataControlLocationIpc for ClipboardCore<T> {
    type Ipc = T;
    const LOCATION: IpcLocation = IpcLocation::Clipboard;

    fn loc_get_device_data(dd: &Device<Self::Ipc>) -> &DeviceData<Offer<Self::Ipc>> {
        &dd.data().clipboard_data
    }

    fn loc_send_selection(dd: &Device<Self::Ipc>, offer: Option<&Rc<Offer<Self::Ipc>>>) {
        dd.send_selection(offer)
    }

    fn loc_unset(seat: &Rc<WlSeatGlobal>) {
        seat.unset_selection()
    }
}

impl<T: DataControlIpc> DataControlLocationIpc for PrimarySelectionCore<T> {
    type Ipc = T;
    const LOCATION: IpcLocation = IpcLocation::PrimarySelection;

    fn loc_get_device_data(dd: &Device<Self::Ipc>) -> &DeviceData<Offer<Self::Ipc>> {
        &dd.data().primary_selection_data
    }

    fn loc_send_selection(dd: &Device<Self::Ipc>, offer: Option<&Rc<Offer<Self::Ipc>>>) {
        dd.send_primary_selection(offer)
    }

    fn loc_unset(seat: &Rc<WlSeatGlobal>) {
        seat.unset_primary_selection()
    }
}

impl<T: DataControlLocationIpc> IpcVtable for DataControlIpcImpl<T> {
    type Device = Device<T::Ipc>;
    type Source = Source<T::Ipc>;
    type Offer = Offer<T::Ipc>;

    fn get_device_data(dd: &Self::Device) -> &DeviceData<Self::Offer> {
        T::loc_get_device_data(dd)
    }

    fn get_device_seat(dd: &Self::Device) -> Rc<WlSeatGlobal> {
        dd.data().seat.clone()
    }

    fn create_offer(
        device: &Rc<Self::Device>,
        offer_data: OfferData<Self::Device>,
    ) -> Result<Rc<Self::Offer>, ClientError> {
        let data = device.data();
        let offer = DataControlOfferData {
            offer_id: data.client.state.data_offer_ids.next(),
            client: data.client.clone(),
            device: device.clone(),
            data: offer_data,
            location: T::LOCATION,
        };
        let rc = T::Ipc::create_offer(data.client.new_id()?, offer);
        data.client.add_server_obj(&rc);
        Ok(rc)
    }

    fn send_selection(dd: &Self::Device, offer: Option<&Rc<Self::Offer>>) {
        T::loc_send_selection(dd, offer)
    }

    fn send_offer(dd: &Self::Device, offer: &Rc<Self::Offer>) {
        dd.send_data_offer(offer);
    }

    fn unset(seat: &Rc<WlSeatGlobal>, _role: Role) {
        T::loc_unset(seat)
    }

    fn device_client(dd: &Rc<Self::Device>) -> &Rc<Client> {
        &dd.data().client
    }
}

impl<T: DataControlSource> DataSource for T {
    fn send_cancelled(&self, _seat: &Rc<WlSeatGlobal>) {
        self.send_cancelled();
    }
}

impl<T: DataControlSource> DynDataSource for T {
    fn source_data(&self) -> &SourceData {
        &self.data().data
    }

    fn send_send(&self, mime_type: &str, fd: Rc<OwnedFd>) {
        self.send_send(mime_type, fd);
    }

    fn offer_to_x(self: Rc<Self>, dd: &Rc<XIpcDevice>) {
        match self.data().location.get() {
            IpcLocation::Clipboard => offer_source_to_x::<XClipboardIpc>(self, dd),
            IpcLocation::PrimarySelection => offer_source_to_x::<XPrimarySelectionIpc>(self, dd),
        }
    }

    fn detach_seat(&self, seat: &Rc<WlSeatGlobal>) {
        detach_seat(self, seat)
    }

    fn cancel_unprivileged_offers(&self) {
        cancel_offers(self, false)
    }
}

impl<T: DataControlOffer> DataOffer for T {
    type Device = Device<T::Ipc>;

    fn offer_data(&self) -> &OfferData<Self::Device> {
        &self.data().data
    }
}

impl<T: DataControlOffer> DynDataOffer for T {
    fn offer_id(&self) -> DataOfferId {
        self.data().offer_id
    }

    fn client_id(&self) -> ClientId {
        self.data().client.id
    }

    fn send_offer(&self, mime_type: &str) {
        self.send_offer(mime_type);
    }

    fn cancel(&self) {
        match self.data().location {
            IpcLocation::Clipboard => cancel_offer::<Clipboard<T::Ipc>>(self),
            IpcLocation::PrimarySelection => cancel_offer::<PrimarySelection<T::Ipc>>(self),
        }
    }

    fn get_seat(&self) -> Rc<WlSeatGlobal> {
        self.data().device.data().seat.clone()
    }

    fn is_privileged(&self) -> bool {
        true
    }
}

pub mod logic {
    use {
        crate::{
            client::ClientError,
            ifs::{
                ipc::{
                    IpcLocation, add_data_source_mime_type, break_device_loops, break_offer_loops,
                    break_source_loops,
                    data_control::private::{
                        Clipboard, DataControlDevice, DataControlOffer, DataControlSource,
                        PrimarySelection, Source, SourceId,
                    },
                    destroy_data_device, destroy_data_offer, destroy_data_source,
                    receive_data_offer,
                },
                wl_seat::WlSeatError,
            },
        },
        std::rc::Rc,
        thiserror::Error,
        uapi::OwnedFd,
    };

    pub fn data_device_break_loops<D: DataControlDevice>(d: &D) {
        break_device_loops::<Clipboard<D::Ipc>>(d);
        break_device_loops::<PrimarySelection<D::Ipc>>(d);
        d.data().seat.remove_data_control_device(d);
    }

    fn use_source<D: DataControlDevice>(
        device: &D,
        source: Option<SourceId<D::Ipc>>,
        location: IpcLocation,
    ) -> Result<Option<Rc<Source<D::Ipc>>>, DataControlError> {
        if let Some(source) = source {
            let src = device.data().client.lookup(source)?;
            if src.data().used.replace(true) {
                return Err(DataControlError::AlreadyUsed);
            }
            src.data().location.set(location);
            Ok(Some(src))
        } else {
            Ok(None)
        }
    }

    pub fn device_set_selection<D: DataControlDevice>(
        d: &D,
        source: Option<SourceId<D::Ipc>>,
    ) -> Result<(), DataControlError> {
        let src = use_source(d, source, IpcLocation::Clipboard)?;
        d.data().seat.set_selection(src)?;
        Ok(())
    }

    pub fn device_destroy<D: DataControlDevice>(d: &D) -> Result<(), DataControlError> {
        destroy_data_device::<Clipboard<D::Ipc>>(d);
        destroy_data_device::<PrimarySelection<D::Ipc>>(d);
        d.data().seat.remove_data_control_device(d);
        d.data().client.remove_obj(d)?;
        Ok(())
    }

    pub fn device_set_primary_selection<D: DataControlDevice>(
        d: &D,
        source: Option<SourceId<D::Ipc>>,
    ) -> Result<(), DataControlError> {
        let src = use_source(d, source, IpcLocation::PrimarySelection)?;
        d.data().seat.set_primary_selection(src)?;
        Ok(())
    }

    pub fn data_source_offer<S: DataControlSource>(
        s: &S,
        mime_type: &str,
    ) -> Result<(), DataControlError> {
        if s.data().used.get() {
            return Err(DataControlError::AlreadyUsed);
        }
        add_data_source_mime_type::<Clipboard<S::Ipc>>(s, mime_type);
        Ok(())
    }

    pub fn data_source_destroy<S: DataControlSource>(s: &S) -> Result<(), DataControlError> {
        match s.data().location.get() {
            IpcLocation::Clipboard => destroy_data_source::<Clipboard<S::Ipc>>(s),
            IpcLocation::PrimarySelection => destroy_data_source::<PrimarySelection<S::Ipc>>(s),
        }
        s.data().data.client.remove_obj(s)?;
        Ok(())
    }

    pub fn data_source_break_loops<S: DataControlSource>(s: &S) {
        match s.data().location.get() {
            IpcLocation::Clipboard => break_source_loops::<Clipboard<S::Ipc>>(s),
            IpcLocation::PrimarySelection => break_source_loops::<PrimarySelection<S::Ipc>>(s),
        }
    }

    pub fn data_offer_receive<O: DataControlOffer>(o: &O, mime_type: &str, fd: Rc<OwnedFd>) {
        match o.data().location {
            IpcLocation::Clipboard => receive_data_offer::<Clipboard<O::Ipc>>(o, mime_type, fd),
            IpcLocation::PrimarySelection => {
                receive_data_offer::<PrimarySelection<O::Ipc>>(o, mime_type, fd)
            }
        }
    }

    pub fn data_offer_destroy<O: DataControlOffer>(o: &O) -> Result<(), DataControlError> {
        match o.data().location {
            IpcLocation::Clipboard => destroy_data_offer::<Clipboard<O::Ipc>>(o),
            IpcLocation::PrimarySelection => destroy_data_offer::<PrimarySelection<O::Ipc>>(o),
        }
        o.data().client.remove_obj(o)?;
        Ok(())
    }

    pub fn data_offer_break_loops<O: DataControlOffer>(o: &O) {
        match o.data().location {
            IpcLocation::Clipboard => break_offer_loops::<Clipboard<O::Ipc>>(o),
            IpcLocation::PrimarySelection => break_offer_loops::<PrimarySelection<O::Ipc>>(o),
        }
    }

    #[derive(Debug, Error)]
    pub enum DataControlError {
        #[error(transparent)]
        ClientError(Box<ClientError>),
        #[error(transparent)]
        WlSeatError(Box<WlSeatError>),
        #[error("The source has already been used")]
        AlreadyUsed,
    }
    efrom!(DataControlError, ClientError);
    efrom!(DataControlError, WlSeatError);
}
