use std::cell::{Cell, RefCell};
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use ahash::AHashSet;
use thiserror::Error;
use uapi::OwnedFd;
use crate::client::{Client, ClientId, WaylandObject};
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::object::ObjectId;
use crate::utils::clonecell::CloneCell;
use crate::utils::smallmap::SmallMap;

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
    type OfferId: Copy + From<ObjectId>;

    type Device;
    type Source;
    type Offer: WaylandObject;

    fn device_id(dd: &Self::Device) -> Self::DeviceId;
    fn get_offer_data(offer: &Self::Offer) -> &OfferData<Self>;
    fn get_source_data(src: &Self::Source) -> &SourceData<Self>;
    fn for_each_device<C>(seat: &WlSeatGlobal, client: ClientId, f: C)
        where C: FnMut(&Rc<Self::Device>);
    fn create_offer(client: &Rc<Client>, data: OfferData<Self>, id: ObjectId) -> Self::Offer;
    fn send_selection(dd: &Self::Device, offer: Self::OfferId);
    fn send_cancelled(source: &Self::Source);
    fn get_offer_id(offer: &Self::Offer) -> Self::OfferId;
    fn send_offer(dd: &Self::Device, offer: &Self::Offer);
    fn send_mime_type(offer: &Self::Offer, mime_type: &str);
    fn unset(seat: &Rc<WlSeatGlobal>);
    fn send_send(src: &Self::Source, mime_type: &str, fd: Rc<OwnedFd>);
}

pub struct OfferData<T: Vtable> {
    device_id: T::DeviceId,
    source: CloneCell<Option<Rc<T::Source>>>,
    client: Rc<Client>,
}

struct Attachment<T: Vtable> {
    seat: Rc<WlSeatGlobal>,
    role: Role,
    offers: SmallMap<T::DeviceId, Rc<T::Offer>, 1>,
}

impl<T: Vtable> Attachment<T> {
    fn detach_offers(&self) {
        while let Some((_, offer)) = self.offers.pop() {
            T::get_offer_data(&offer).source.set(None);
        }
    }
}

#[derive(Debug, Error)]
pub enum IpcError {
    #[error("The data source is already attached")]
    AlreadyAttached,
}

pub struct SourceData<T: Vtable> {
    attachment: RefCell<Option<Attachment<T>>>,
    disconnecting: Cell<bool>,
    mime_types: RefCell<AHashSet<String>>,
    client: Rc<Client>
}

impl<T: Vtable> SourceData<T> {
    fn new(client: &Rc<Client>) -> Self {
        Self {
            attachment: Default::default(),
            disconnecting: Cell::new(false),
            mime_types: Default::default(),
            client: client.clone(),
        }
    }
}

pub fn attach_source<T: Vtable>(
    src: &T::Source,
    seat: &Rc<WlSeatGlobal>,
    role: Role,
) -> Result<(), IpcError> {
    let src = T::get_source_data(src);
    let mut attachment = src.attachment.borrow_mut();
    if attachment.is_some() {
        return Err(IpcError::AlreadyAttached);
    }
    *attachment = Some(Attachment {
        seat: seat.clone(),
        role,
        offers: Default::default(),
    });
    Ok(())
}

pub fn detach_source<T: Vtable>(src: &T::Source) {
    let data = T::get_source_data(src);
    if data.disconnecting.get() {
        return;
    }
    if let Some(attachment) = data.attachment.borrow_mut().take() {
        attachment.detach_offers();
    }
    T::send_cancelled(src);
}

pub fn offer_source_to<T: Vtable>(src: &Rc<T::Source>, client: &Rc<Client>) {
    let data = T::get_source_data(src);
    let mut attachment = data.attachment.borrow_mut();
    let attachment = match attachment.deref_mut() {
        Some(a) => a,
        _ => {
            log::error!("Trying to create an offer from a unattached data source");
            return;
        }
    };
    attachment.detach_offers();
    T::for_each_device(&attachment.seat, client.id, |dd| {
        let id = match client.new_id() {
            Ok(id) => id,
            Err(e) => {
                client.error(e);
                return;
            }
        };
        let offer_data = OfferData {
            device_id: T::device_id(dd),
            source: CloneCell::new(Some(src.clone())),
            client: client.clone(),
        };
        let offer = Rc::new(T::create_offer(client, offer_data, id));
        attachment.offers.insert(T::device_id(dd), offer.clone());
        let mt = data.mime_types.borrow_mut();
        T::send_offer(dd, &offer);
        for mt in mt.deref() {
            T::send_mime_type(&offer, mt);
        }
        if attachment.role == Role::Selection {
            T::send_selection(dd, T::get_offer_id(&offer));
        }
        client.add_server_obj(&offer);
    });
}


fn add_mime_type<T: Vtable>(src: &T::Source, mime_type: &str) {
    let data = T::get_source_data(src);
    if data
        .mime_types
        .borrow_mut()
        .insert(mime_type.to_string())
    {
        if let Some(attachment) = data.attachment.borrow_mut().deref_mut() {
            for (_, offer) in &attachment.offers {
                T::send_mime_type(&offer, mime_type);
                let data = T::get_offer_data(&offer);
                data.client.flush();
            }
        }
    }
}

fn disconnect_source<T: Vtable>(src: &T::Source) {
    let data = T::get_source_data(src);
    data.disconnecting.set(true);
    if let Some(attachment) = data.attachment.borrow_mut().take() {
        attachment.detach_offers();
        T::unset(&attachment.seat);
    }
}

fn disconnect_offer<T: Vtable>(offer: &T::Offer) {
    let data = T::get_offer_data(offer);
    if let Some(src) = data.source.set(None) {
        let src_data = T::get_source_data(&src);
        if let Some(attachment) = src_data.attachment.borrow_mut().deref_mut() {
            attachment.offers.remove(&data.device_id);
        }
    }
}

fn receive<T: Vtable>(offer: &T::Offer, mime_type: &str, fd: Rc<OwnedFd>) {
    let data = T::get_offer_data(offer);
    if let Some(src) = data.source.get() {
        T::send_send(&src, mime_type, fd);
        let data = T::get_source_data(&src);
        data.client.flush();
    }
}
