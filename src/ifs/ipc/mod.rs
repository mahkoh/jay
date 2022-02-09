use crate::client::{Client, ClientId, WaylandObject};
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::object::ObjectId;
use crate::utils::clonecell::CloneCell;
use crate::utils::smallmap::SmallMap;
use ahash::AHashSet;
use std::cell::{Cell, RefCell};
use std::ops::{Deref};
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;
use crate::NumCell;
use crate::utils::bitflags::BitflagsExt;

pub mod wl_data_device;
pub mod wl_data_device_manager;
pub mod wl_data_offer;
pub mod wl_data_source;
pub mod zwp_primary_selection_device_manager_v1;
pub mod zwp_primary_selection_device_v1;
pub mod zwp_primary_selection_offer_v1;
pub mod zwp_primary_selection_source_v1;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Role {
    Selection,
    Dnd,
}

pub trait Vtable: Sized {
    type DeviceId: Eq + Copy;
    type OfferId: Eq + Copy + From<ObjectId>;

    type Device;
    type Source;
    type Offer: WaylandObject;

    fn device_id(dd: &Self::Device) -> Self::DeviceId;
    fn get_device_data(dd: &Self::Device) -> &DeviceData<Self>;
    fn get_offer_data(offer: &Self::Offer) -> &OfferData<Self>;
    fn get_source_data(src: &Self::Source) -> &SourceData<Self>;
    fn for_each_device<C>(seat: &WlSeatGlobal, client: ClientId, f: C)
    where
        C: FnMut(&Rc<Self::Device>);
    fn create_offer(
        client: &Rc<Client>,
        dd: &Rc<Self::Device>,
        data: OfferData<Self>,
        id: ObjectId,
    ) -> Self::Offer;
    fn send_selection(dd: &Self::Device, offer: Self::OfferId);
    fn send_cancelled(source: &Self::Source);
    fn get_offer_id(offer: &Self::Offer) -> Self::OfferId;
    fn send_offer(dd: &Self::Device, offer: &Self::Offer);
    fn send_mime_type(offer: &Self::Offer, mime_type: &str);
    fn unset(seat: &Rc<WlSeatGlobal>, role: Role);
    fn send_send(src: &Self::Source, mime_type: &str, fd: Rc<OwnedFd>);
}

pub struct DeviceData<T: Vtable> {
    selection: CloneCell<Option<Rc<T::Offer>>>,
    dnd: CloneCell<Option<Rc<T::Offer>>>,
}

impl<T: Vtable> Default for DeviceData<T> {
    fn default() -> Self {
        Self {
            selection: Default::default(),
            dnd: Default::default(),
        }
    }
}

pub struct OfferData<T: Vtable> {
    device: CloneCell<Option<Rc<T::Device>>>,
    source: CloneCell<Option<Rc<T::Source>>>,
    client: Rc<Client>,
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

pub struct SourceData<T: Vtable> {
    seat: CloneCell<Option<Rc<WlSeatGlobal>>>,
    offers: SmallMap<T::OfferId, Rc<T::Offer>, 1>,
    offer_client: Cell<ClientId>,
    mime_types: RefCell<AHashSet<String>>,
    client: Rc<Client>,
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
            selected_action: Cell::new(0)
        }
    }
}

impl<T: Vtable> SourceData<T> {
    fn new(client: &Rc<Client>) -> Self {
        Self {
            seat: Default::default(),
            offers: Default::default(),
            offer_client: Cell::new(client.id),
            mime_types: Default::default(),
            client: client.clone(),
            state: NumCell::new(0),
            actions: Cell::new(None),
            role: Cell::new(Role::Selection),
            shared: Default::default(),
        }
    }
}

pub fn attach_seat<T: Vtable>(
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
        if data.actions.get().is_none() {
            return Err(IpcError::ActionsNotSet);
        }
    } else {
        if data.actions.get().is_some() {
            return Err(IpcError::ActionsSet);
        }
    }
    data.state.set(state);
    data.role.set(role);
    data.seat.set(Some(seat.clone()));
    Ok(())
}

pub fn cancel_offers<T: Vtable>(src: &T::Source) {
    let data = T::get_source_data(src);
    while let Some((_, offer)) = data.offers.pop() {
        let data = T::get_offer_data(&offer);
        log::error!("cancel_offers");
        data.source.take();
        destroy_offer::<T>(&offer);
    }
}

pub fn detach_seat<T: Vtable>(src: &T::Source) {
    let data = T::get_source_data(src);
    data.seat.set(None);
    cancel_offers::<T>(src);
    if !data.state.get().contains(SOURCE_STATE_FINISHED) {
        T::send_cancelled(src);
    }
}

pub fn offer_source_to<T: Vtable>(src: &Rc<T::Source>, client: &Rc<Client>) {
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
        let id = match client.new_id() {
            Ok(id) => id,
            Err(e) => {
                client.error(e);
                return;
            }
        };
        let device_data = T::get_device_data(dd);
        let offer_data = OfferData {
            device: CloneCell::new(Some(dd.clone())),
            source: CloneCell::new(Some(src.clone())),
            client: client.clone(),
            shared: shared.clone(),
        };
        let offer = Rc::new(T::create_offer(client, dd, offer_data, id));
        data.offers.insert(id.into(), offer.clone());
        let mt = data.mime_types.borrow_mut();
        T::send_offer(dd, &offer);
        for mt in mt.deref() {
            T::send_mime_type(&offer, mt);
        }
        match data.role.get() {
            Role::Selection => {
                T::send_selection(dd, T::get_offer_id(&offer));
                device_data.selection.set(Some(offer.clone()));
            }
            Role::Dnd => {
                device_data.dnd.set(Some(offer.clone()));
            }
        }
        client.add_server_obj(&offer);
    });
}

fn add_mime_type<T: Vtable>(src: &T::Source, mime_type: &str) {
    let data = T::get_source_data(src);
    if data.mime_types.borrow_mut().insert(mime_type.to_string()) {
        for (_, offer) in &data.offers {
            T::send_mime_type(&offer, mime_type);
            let data = T::get_offer_data(&offer);
            data.client.flush();
        }
    }
}

fn destroy_source<T: Vtable>(src: &T::Source) {
    let data = T::get_source_data(src);
    if let Some(seat) = data.seat.take() {
        T::unset(&seat, data.role.get());
    }
}

fn destroy_offer<T: Vtable>(offer: &T::Offer) {
    let data = T::get_offer_data(offer);
    if let Some(device) = data.device.take() {
        let device_data = T::get_device_data(&device);
        match data.shared.role.get() {
            Role::Selection => {
                T::send_selection(&device, ObjectId::NONE.into());
                device_data.selection.take();
            }
            Role::Dnd => {
                device_data.dnd.take();
            }
        }
    }
    log::error!("destroy_offer");
    if let Some(src) = data.source.take() {
        let src_data = T::get_source_data(&src);
        src_data.offers.remove(&T::get_offer_id(offer));
        if src_data.offers.is_empty() && src_data.role.get() == Role::Dnd && data.shared.state.get().contains(OFFER_STATE_DROPPED) {
            if let Some(seat) = src_data.seat.take() {
                T::unset(&seat, data.shared.role.get());
            }
        }
    }
}

fn destroy_device<T: Vtable>(dd: &T::Device) {
    let data = T::get_device_data(dd);
    let offers = [data.selection.take(), data.dnd.take()];
    for offer in offers.into_iter().flat_map(|o| o.into_iter()) {
        T::get_offer_data(&offer).device.take();
        destroy_offer::<T>(&offer);
    }
}

fn break_source_loops<T: Vtable>(src: &T::Source) {
    let data = T::get_source_data(src);
    if data.offer_client.get() == data.client.id {
        data.offers.take();
    }
    destroy_source::<T>(src);
}

fn break_offer_loops<T: Vtable>(offer: &T::Offer) {
    let data = T::get_offer_data(offer);
    data.device.set(None);
    destroy_offer::<T>(offer);
}

fn break_device_loops<T: Vtable>(dd: &T::Device) {
    let data = T::get_device_data(dd);
    data.selection.take();
    data.dnd.take();
    destroy_device::<T>(dd);
}

fn receive<T: Vtable>(offer: &T::Offer, mime_type: &str, fd: Rc<OwnedFd>) {
    let data = T::get_offer_data(offer);
    if let Some(src) = data.source.get() {
        T::send_send(&src, mime_type, fd);
        let data = T::get_source_data(&src);
        data.client.flush();
    }
}
