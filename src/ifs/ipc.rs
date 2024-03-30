use {
    crate::{
        client::{Client, ClientError, ClientId, WaylandObject},
        ifs::wl_seat::{WlSeatError, WlSeatGlobal},
        utils::{
            bitflags::BitflagsExt, cell_ext::CellExt, clonecell::CloneCell, numcell::NumCell,
            smallmap::SmallMap,
        },
    },
    ahash::AHashSet,
    std::{
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
pub mod zwp_primary_selection_device_manager_v1;
pub mod zwp_primary_selection_device_v1;
pub mod zwp_primary_selection_offer_v1;
pub mod zwp_primary_selection_source_v1;

linear_ids!(DataSourceIds, DataSourceId, u64);
linear_ids!(DataOfferIds, DataOfferId, u64);

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Role {
    Selection,
    Dnd,
}

pub trait IpcVtable: Sized {
    type Device;
    type Source;
    type Offer: WaylandObject;

    fn get_device_data(dd: &Self::Device) -> &DeviceData<Self>;
    fn get_device_seat(dd: &Self::Device) -> Rc<WlSeatGlobal>;
    fn create_xwm_source(client: &Rc<Client>) -> Self::Source;
    fn set_seat_selection(
        seat: &Rc<WlSeatGlobal>,
        source: &Rc<Self::Source>,
        serial: Option<u32>,
    ) -> Result<(), WlSeatError>;
    fn get_offer_data(offer: &Self::Offer) -> &OfferData<Self>;
    fn get_source_data(src: &Self::Source) -> &SourceData<Self>;
    fn for_each_device<C>(seat: &WlSeatGlobal, client: ClientId, f: C)
    where
        C: FnMut(&Rc<Self::Device>);
    fn create_offer(
        client: &Rc<Client>,
        dd: &Rc<Self::Device>,
        data: OfferData<Self>,
    ) -> Result<Rc<Self::Offer>, ClientError>;
    fn send_selection(dd: &Self::Device, offer: Option<&Rc<Self::Offer>>);
    fn send_cancelled(source: &Rc<Self::Source>, seat: &Rc<WlSeatGlobal>);
    fn get_offer_id(offer: &Self::Offer) -> DataOfferId;
    fn send_offer(dd: &Self::Device, offer: &Rc<Self::Offer>);
    fn send_mime_type(offer: &Rc<Self::Offer>, mime_type: &str);
    fn unset(seat: &Rc<WlSeatGlobal>, role: Role);
    fn send_send(src: &Rc<Self::Source>, mime_type: &str, fd: Rc<OwnedFd>);
    fn remove_from_seat(device: &Self::Device);
    fn get_offer_seat(offer: &Self::Offer) -> Rc<WlSeatGlobal>;
}

pub struct DeviceData<T: IpcVtable> {
    selection: CloneCell<Option<Rc<T::Offer>>>,
    dnd: CloneCell<Option<Rc<T::Offer>>>,
    pub is_xwm: bool,
}

pub struct OfferData<T: IpcVtable> {
    device: CloneCell<Option<Rc<T::Device>>>,
    source: CloneCell<Option<Rc<T::Source>>>,
    shared: Rc<SharedState>,
    pub is_xwm: bool,
}

impl<T: IpcVtable> OfferData<T> {
    pub fn source(&self) -> Option<Rc<T::Source>> {
        self.source.get()
    }
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

pub struct SourceData<T: IpcVtable> {
    pub seat: CloneCell<Option<Rc<WlSeatGlobal>>>,
    pub id: DataSourceId,
    offers: SmallMap<DataOfferId, Rc<T::Offer>, 1>,
    offer_client: Cell<ClientId>,
    mime_types: RefCell<AHashSet<String>>,
    pub client: Rc<Client>,
    state: NumCell<u32>,
    actions: Cell<Option<u32>>,
    role: Cell<Role>,
    shared: CloneCell<Rc<SharedState>>,
    pub is_xwm: bool,
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

impl<T: IpcVtable> SourceData<T> {
    fn new(client: &Rc<Client>, is_xwm: bool) -> Self {
        Self {
            seat: Default::default(),
            id: client.state.data_source_ids.next(),
            offers: Default::default(),
            offer_client: Cell::new(client.id),
            mime_types: Default::default(),
            client: client.clone(),
            state: NumCell::new(0),
            actions: Cell::new(None),
            role: Cell::new(Role::Selection),
            shared: Default::default(),
            is_xwm,
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

pub fn attach_seat<T: IpcVtable>(
    src: &T::Source,
    seat: &Rc<WlSeatGlobal>,
    role: Role,
) -> Result<(), IpcError> {
    let data = T::get_source_data(src);
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

pub fn cancel_offers<T: IpcVtable>(src: &T::Source) {
    let data = T::get_source_data(src);
    while let Some((_, offer)) = data.offers.pop() {
        let data = T::get_offer_data(&offer);
        data.source.take();
        destroy_data_offer::<T>(&offer);
    }
}

pub fn detach_seat<T: IpcVtable>(src: &Rc<T::Source>, seat: &Rc<WlSeatGlobal>) {
    let data = T::get_source_data(src);
    data.seat.set(None);
    cancel_offers::<T>(src);
    if !data.state.get().contains(SOURCE_STATE_FINISHED) {
        T::send_cancelled(src, seat);
    }
    // data.client.flush();
}

pub fn offer_source_to<T: IpcVtable>(src: &Rc<T::Source>, client: &Rc<Client>) {
    let data = T::get_source_data(src);
    let seat = match data.seat.get() {
        Some(a) => a,
        _ => {
            log::error!("Trying to create an offer from a unattached data source");
            return;
        }
    };
    cancel_offers::<T>(src);
    data.offer_client.set(client.id);
    let shared = data.shared.get();
    shared.role.set(data.role.get());
    T::for_each_device(&seat, client.id, |dd| {
        let device_data = T::get_device_data(dd);
        let offer_data = OfferData {
            device: CloneCell::new(Some(dd.clone())),
            source: CloneCell::new(Some(src.clone())),
            shared: shared.clone(),
            is_xwm: device_data.is_xwm,
        };
        let offer = match T::create_offer(client, dd, offer_data) {
            Ok(o) => o,
            Err(e) => {
                client.error(e);
                return;
            }
        };
        data.offers.insert(T::get_offer_id(&offer), offer.clone());
        let mt = data.mime_types.borrow_mut();
        T::send_offer(dd, &offer);
        for mt in mt.deref() {
            T::send_mime_type(&offer, mt);
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
        if !device_data.is_xwm {
            client.add_server_obj(&offer);
        }
    });
}

pub fn add_data_source_mime_type<T: IpcVtable>(src: &T::Source, mime_type: &str) {
    let data = T::get_source_data(src);
    if data.mime_types.borrow_mut().insert(mime_type.to_string()) {
        for (_, offer) in &data.offers {
            T::send_mime_type(&offer, mime_type);
            // let data = T::get_offer_data(&offer);
            // data.client.flush();
        }
    }
}

pub fn destroy_data_source<T: IpcVtable>(src: &T::Source) {
    let data = T::get_source_data(src);
    if let Some(seat) = data.seat.take() {
        T::unset(&seat, data.role.get());
    }
}

pub fn destroy_data_offer<T: IpcVtable>(offer: &T::Offer) {
    let data = T::get_offer_data(offer);
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
        let src_data = T::get_source_data(&src);
        src_data.offers.remove(&T::get_offer_id(offer));
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
        T::get_offer_data(&offer).device.take();
        destroy_data_offer::<T>(&offer);
    }
}

fn break_source_loops<T: IpcVtable>(src: &T::Source) {
    let data = T::get_source_data(src);
    if data.offer_client.get() == data.client.id {
        data.offers.take();
    }
    destroy_data_source::<T>(src);
}

fn break_offer_loops<T: IpcVtable>(offer: &T::Offer) {
    let data = T::get_offer_data(offer);
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
    let data = T::get_offer_data(offer);
    if let Some(src) = data.source.get() {
        T::send_send(&src, mime_type, fd);
        // let data = T::get_source_data(&src);
        // data.client.flush();
    }
}
