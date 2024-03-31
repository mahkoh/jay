use {
    crate::{
        client::{Client, ClientError, ClientId},
        fixed::Fixed,
        ifs::{
            ipc::{
                x_data_device::XIpcDevice, zwlr_data_control_device_v1::ZwlrDataControlDeviceV1,
            },
            wl_seat::{WlSeatError, WlSeatGlobal},
        },
        utils::{
            bitflags::BitflagsExt, cell_ext::CellExt, clonecell::CloneCell, numcell::NumCell,
            smallmap::SmallMap,
        },
        wire::WlSurfaceId,
    },
    ahash::AHashSet,
    smallvec::SmallVec,
    std::{
        any,
        cell::{Cell, RefCell},
        ops::Deref,
        rc::Rc,
    },
    thiserror::Error,
    uapi::OwnedFd,
};

pub mod wl_data_device;
pub mod wl_data_device_manager;
pub mod wl_data_offer;
pub mod wl_data_source;
pub mod x_data_device;
pub mod x_data_offer;
pub mod x_data_source;
pub mod zwlr_data_control_device_v1;
pub mod zwlr_data_control_manager_v1;
pub mod zwlr_data_control_offer_v1;
pub mod zwlr_data_control_source_v1;
pub mod zwp_primary_selection_device_manager_v1;
pub mod zwp_primary_selection_device_v1;
pub mod zwp_primary_selection_offer_v1;
pub mod zwp_primary_selection_source_v1;

linear_ids!(DataSourceIds, DataSourceId, u64);
linear_ids!(DataOfferIds, DataOfferId, u64);

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum IpcLocation {
    Clipboard,
    PrimarySelection,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Role {
    Selection,
    Dnd,
}

pub trait DataSource: DynDataSource {
    fn send_cancelled(&self, seat: &Rc<WlSeatGlobal>);
}

pub trait DynDataSource: 'static {
    fn source_data(&self) -> &SourceData;
    fn send_send(&self, mime_type: &str, fd: Rc<OwnedFd>);
    fn offer_to_regular_client(self: Rc<Self>, client: &Rc<Client>);
    fn offer_to_x(self: Rc<Self>, dd: &Rc<XIpcDevice>);
    fn offer_to_wlr_device(self: Rc<Self>, dd: &Rc<ZwlrDataControlDeviceV1>);
    fn detach_seat(&self, seat: &Rc<WlSeatGlobal>);
    fn cancel_unprivileged_offers(&self);

    fn send_target(&self, mime_type: Option<&str>) {
        let _ = mime_type;
        log::warn!(
            "send_target called on data source of type {}",
            any::type_name_of_val(self)
        )
    }
    fn send_dnd_finished(&self) {
        log::warn!(
            "send_dnd_finished called on data source of type {}",
            any::type_name_of_val(self)
        )
    }
    fn update_selected_action(&self) {
        log::warn!(
            "update_selected_action called on data source of type {}",
            any::type_name_of_val(self)
        )
    }
}

pub trait DataOffer: DynDataOffer {
    type Device;

    fn offer_data(&self) -> &OfferData<Self::Device>;
}

pub trait DynDataOffer: 'static {
    fn offer_id(&self) -> DataOfferId;
    fn client_id(&self) -> ClientId;
    fn send_offer(&self, mime_type: &str);
    fn destroy(&self);
    fn cancel(&self);
    fn get_seat(&self) -> Rc<WlSeatGlobal>;

    fn is_privileged(&self) -> bool {
        false
    }

    fn send_action(&self, action: u32) {
        let _ = action;
        log::warn!(
            "send_action called on data source of type {}",
            any::type_name_of_val(self)
        )
    }
    fn send_enter(&self, surface: WlSurfaceId, x: Fixed, y: Fixed, serial: u32) {
        let _ = surface;
        let _ = x;
        let _ = y;
        let _ = serial;
        log::warn!(
            "send_enter called on data source of type {}",
            any::type_name_of_val(self)
        )
    }
    fn send_source_actions(&self) {
        log::warn!(
            "send_source_actions called on data source of type {}",
            any::type_name_of_val(self)
        )
    }
}

pub trait IterableIpcVtable: IpcVtable {
    fn for_each_device<C>(seat: &WlSeatGlobal, client: ClientId, f: C)
    where
        C: FnMut(&Rc<Self::Device>);
}

pub trait WlrIpcVtable: IpcVtable<Device = ZwlrDataControlDeviceV1> {
    fn for_each_device<C>(seat: &WlSeatGlobal, f: C)
    where
        C: FnMut(&Rc<Self::Device>);
}

pub trait IpcVtable: Sized {
    const LOCATION: IpcLocation;

    type Device;
    type Source: DataSource;
    type Offer: DataOffer<Device = Self::Device>;

    fn get_device_data(dd: &Self::Device) -> &DeviceData<Self::Offer>;
    fn get_device_seat(dd: &Self::Device) -> Rc<WlSeatGlobal>;
    fn set_seat_selection(
        seat: &Rc<WlSeatGlobal>,
        source: &Rc<Self::Source>,
        serial: Option<u32>,
    ) -> Result<(), WlSeatError>;
    fn create_offer(
        dd: &Rc<Self::Device>,
        data: OfferData<Self::Device>,
    ) -> Result<Rc<Self::Offer>, ClientError>;
    fn send_selection(dd: &Self::Device, offer: Option<&Rc<Self::Offer>>);
    fn send_offer(dd: &Self::Device, offer: &Rc<Self::Offer>);
    fn unset(seat: &Rc<WlSeatGlobal>, role: Role);
    fn device_client(dd: &Rc<Self::Device>) -> &Rc<Client>;
}

pub struct DeviceData<O> {
    selection: CloneCell<Option<Rc<O>>>,
    dnd: CloneCell<Option<Rc<O>>>,
}

impl<O> Default for DeviceData<O> {
    fn default() -> Self {
        Self {
            selection: Default::default(),
            dnd: Default::default(),
        }
    }
}

pub struct OfferData<D> {
    device: CloneCell<Option<Rc<D>>>,
    source: CloneCell<Option<Rc<dyn DynDataSource>>>,
    shared: Rc<SharedState>,
}

#[derive(Debug, Error)]
pub enum IpcError {
    #[error("The data source is already attached")]
    AlreadyAttached,
    #[error("The data source does not have drag-and-drop actions set")]
    ActionsNotSet,
    #[error("The data source has drag-and-drop actions set")]
    ActionsSet,
}

const OFFER_STATE_ACCEPTED: u32 = 1 << 0;
const OFFER_STATE_FINISHED: u32 = 1 << 1;
const OFFER_STATE_DROPPED: u32 = 1 << 2;

const SOURCE_STATE_USED: u32 = 1 << 1;
const SOURCE_STATE_FINISHED: u32 = 1 << 2;
const SOURCE_STATE_DROPPED: u32 = 1 << 3;
const SOURCE_STATE_CANCELLED: u32 = 1 << 4;
const SOURCE_STATE_DROPPED_OR_CANCELLED: u32 = SOURCE_STATE_DROPPED | SOURCE_STATE_CANCELLED;

pub struct SourceData {
    pub seat: CloneCell<Option<Rc<WlSeatGlobal>>>,
    pub id: DataSourceId,
    offers: SmallMap<DataOfferId, Rc<dyn DynDataOffer>, 1>,
    mime_types: RefCell<AHashSet<String>>,
    pub client: Rc<Client>,
    state: NumCell<u32>,
    actions: Cell<Option<u32>>,
    role: Cell<Role>,
    shared: CloneCell<Rc<SharedState>>,
}

struct SharedState {
    state: NumCell<u32>,
    role: Cell<Role>,
    receiver_actions: Cell<u32>,
    receiver_preferred_action: Cell<u32>,
    selected_action: Cell<u32>,
}

impl Default for SharedState {
    fn default() -> Self {
        Self {
            state: NumCell::new(0),
            role: Cell::new(Role::Selection),
            receiver_actions: Cell::new(0),
            receiver_preferred_action: Cell::new(0),
            selected_action: Cell::new(0),
        }
    }
}

impl SourceData {
    pub fn new(client: &Rc<Client>) -> Self {
        Self {
            seat: Default::default(),
            id: client.state.data_source_ids.next(),
            offers: Default::default(),
            mime_types: Default::default(),
            client: client.clone(),
            state: NumCell::new(0),
            actions: Cell::new(None),
            role: Cell::new(Role::Selection),
            shared: Default::default(),
        }
    }

    pub fn was_used(&self) -> bool {
        self.state.get().contains(SOURCE_STATE_USED)
    }

    pub fn was_dropped_or_cancelled(&self) -> bool {
        self.state
            .get()
            .intersects(SOURCE_STATE_DROPPED_OR_CANCELLED)
    }
}

pub fn attach_seat<S: DynDataSource>(
    src: &S,
    seat: &Rc<WlSeatGlobal>,
    role: Role,
) -> Result<(), IpcError> {
    let data = src.source_data();
    let mut state = data.state.get();
    if state.contains(SOURCE_STATE_USED) {
        return Err(IpcError::AlreadyAttached);
    }
    state |= SOURCE_STATE_USED;
    if role == Role::Dnd {
        if data.actions.is_none() {
            return Err(IpcError::ActionsNotSet);
        }
    } else {
        if data.actions.is_some() {
            return Err(IpcError::ActionsSet);
        }
    }
    data.state.set(state);
    data.role.set(role);
    data.seat.set(Some(seat.clone()));
    Ok(())
}

pub fn cancel_offers<S: DynDataSource>(src: &S, cancel_privileged: bool) {
    let data = src.source_data();
    let mut offers = data.offers.take();
    offers.retain(|o| {
        let retain = !cancel_privileged && o.1.is_privileged();
        if !retain {
            o.1.cancel();
        }
        retain
    });
    data.offers.replace(offers);
}

pub fn cancel_offer<T: IpcVtable>(offer: &T::Offer) {
    let data = offer.offer_data();
    data.source.take();
    destroy_data_offer::<T>(&offer);
}

pub fn detach_seat<S: DataSource>(src: &S, seat: &Rc<WlSeatGlobal>) {
    let data = src.source_data();
    data.seat.set(None);
    cancel_offers(src, true);
    if !data.state.get().contains(SOURCE_STATE_FINISHED) {
        src.send_cancelled(seat);
    }
    // data.client.flush();
}

fn offer_source_to_device<T: IpcVtable, S: DynDataSource>(
    src: &Rc<S>,
    dd: &Rc<T::Device>,
    data: &SourceData,
    shared: Rc<SharedState>,
) {
    let device_data = T::get_device_data(dd);
    let offer_data = OfferData {
        device: CloneCell::new(Some(dd.clone())),
        source: CloneCell::new(Some(src.clone())),
        shared: shared.clone(),
    };
    let offer = match T::create_offer(dd, offer_data) {
        Ok(o) => o,
        Err(e) => {
            T::device_client(dd).error(e);
            return;
        }
    };
    data.offers.insert(offer.offer_id(), offer.clone());
    let mt = data.mime_types.borrow_mut();
    T::send_offer(dd, &offer);
    for mt in mt.deref() {
        offer.clone().send_offer(mt);
    }
    match data.role.get() {
        Role::Selection => {
            T::send_selection(dd, Some(&offer));
            device_data.selection.set(Some(offer.clone()));
        }
        Role::Dnd => {
            device_data.dnd.set(Some(offer.clone()));
        }
    }
}

fn offer_source_to_x<T, S>(src: &Rc<S>, dd: &Rc<XIpcDevice>)
where
    T: IpcVtable<Device = XIpcDevice>,
    S: DynDataSource,
{
    let data = src.source_data();
    src.cancel_unprivileged_offers();
    let shared = data.shared.get();
    shared.role.set(data.role.get());
    offer_source_to_device::<T, S>(src, dd, data, shared);
}

pub fn offer_source_to_wlr_device<T, S>(src: &Rc<S>, dd: &Rc<T::Device>)
where
    T: IpcVtable<Device = ZwlrDataControlDeviceV1>,
    S: DynDataSource,
{
    let data = src.source_data();
    let shared = data.shared.get();
    shared.role.set(data.role.get());
    offer_source_to_device::<T, _>(src, dd, data, shared);
}

fn offer_source_to_regular_client<T: IterableIpcVtable, S: DynDataSource>(
    src: &Rc<S>,
    client: &Rc<Client>,
) {
    let data = src.source_data();
    let seat = match data.seat.get() {
        Some(a) => a,
        _ => {
            log::error!("Trying to create an offer from a unattached data source");
            return;
        }
    };
    src.cancel_unprivileged_offers();
    let shared = data.shared.get();
    shared.role.set(data.role.get());
    T::for_each_device(&seat, client.id, |dd| {
        offer_source_to_device::<T, S>(src, dd, data, shared.clone());
    });
}

pub fn add_data_source_mime_type<T: IpcVtable>(src: &T::Source, mime_type: &str) {
    let data = src.source_data();
    if data.mime_types.borrow_mut().insert(mime_type.to_string()) {
        for (_, offer) in &data.offers {
            offer.send_offer(mime_type);
            // let data = T::get_offer_data(&offer);
            // data.client.flush();
        }
    }
}

pub fn destroy_data_source<T: IpcVtable>(src: &T::Source) {
    let data = src.source_data();
    if let Some(seat) = data.seat.take() {
        T::unset(&seat, data.role.get());
    }
}

pub fn destroy_data_offer<T: IpcVtable>(offer: &T::Offer) {
    let data = offer.offer_data();
    if let Some(device) = data.device.take() {
        let device_data = T::get_device_data(&device);
        match data.shared.role.get() {
            Role::Selection => {
                T::send_selection(&device, None);
                device_data.selection.take();
            }
            Role::Dnd => {
                device_data.dnd.take();
            }
        }
    }
    if let Some(src) = data.source.take() {
        let src_data = src.source_data();
        src_data.offers.remove(&offer.offer_id());
        if src_data.offers.is_empty()
            && src_data.role.get() == Role::Dnd
            && data.shared.state.get().contains(OFFER_STATE_DROPPED)
        {
            if let Some(seat) = src_data.seat.take() {
                T::unset(&seat, data.shared.role.get());
            }
        }
    }
}

pub fn destroy_data_device<T: IpcVtable>(dd: &T::Device) {
    let data = T::get_device_data(dd);
    let offers = [data.selection.take(), data.dnd.take()];
    for offer in offers.into_iter().flat_map(|o| o.into_iter()) {
        offer.offer_data().device.take();
        destroy_data_offer::<T>(&offer);
    }
}

fn break_source_loops<T: IpcVtable>(src: &T::Source) {
    let data = src.source_data();
    let mut remove = SmallVec::<[DataOfferId; 1]>::new();
    for (id, offer) in &data.offers {
        if offer.client_id() == data.client.id {
            remove.push(id);
        }
    }
    while let Some(id) = remove.pop() {
        data.offers.remove(&id);
    }
    destroy_data_source::<T>(src);
}

fn break_offer_loops<T: IpcVtable>(offer: &T::Offer) {
    let data = offer.offer_data();
    data.device.set(None);
    destroy_data_offer::<T>(offer);
}

fn break_device_loops<T: IpcVtable>(dd: &T::Device) {
    let data = T::get_device_data(dd);
    data.selection.take();
    data.dnd.take();
    destroy_data_device::<T>(dd);
}

pub fn receive_data_offer<T: IpcVtable>(offer: &T::Offer, mime_type: &str, fd: Rc<OwnedFd>) {
    let data = offer.offer_data();
    if let Some(src) = data.source.get() {
        src.send_send(mime_type, fd);
        // let data = T::get_source_data(&src);
        // data.client.flush();
    }
}
