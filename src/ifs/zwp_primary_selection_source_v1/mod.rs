mod types;

use crate::client::{Client, DynEventFormatter};
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::ifs::zwp_primary_selection_offer_v1::ZwpPrimarySelectionOfferV1;
use crate::object::Object;
use crate::utils::buffd::MsgParser;
use crate::utils::clonecell::CloneCell;
use ahash::AHashSet;
use std::cell::RefCell;
use std::rc::Rc;
pub use types::*;
use uapi::OwnedFd;

const OFFER: u32 = 0;
const DESTROY: u32 = 1;

const SEND: u32 = 0;
const CANCELLED: u32 = 1;

id!(ZwpPrimarySelectionSourceV1Id);

pub struct ZwpPrimarySelectionSourceV1 {
    pub id: ZwpPrimarySelectionSourceV1Id,
    pub client: Rc<Client>,
    pub mime_types: RefCell<AHashSet<String>>,
    seat: CloneCell<Option<Rc<WlSeatGlobal>>>,
    offer: CloneCell<Option<Rc<ZwpPrimarySelectionOfferV1>>>,
}

impl ZwpPrimarySelectionSourceV1 {
    pub fn new(id: ZwpPrimarySelectionSourceV1Id, client: &Rc<Client>) -> Self {
        Self {
            id,
            client: client.clone(),
            mime_types: RefCell::new(Default::default()),
            seat: Default::default(),
            offer: Default::default(),
        }
    }

    pub fn attach(&self, seat: &Rc<WlSeatGlobal>) -> Result<(), ZwpPrimarySelectionSourceV1Error> {
        if self.seat.get().is_some() {
            return Err(ZwpPrimarySelectionSourceV1Error::AlreadyAttached);
        }
        self.seat.set(Some(seat.clone()));
        Ok(())
    }

    pub fn detach(self: &Rc<Self>) {
        self.seat.set(None);
        if let Some(offer) = self.offer.set(None) {
            offer.source.set(None);
        }
        self.client.event(self.cancelled());
        self.client.flush();
    }

    pub fn create_offer(self: &Rc<Self>, client: &Rc<Client>) {
        let seat = match self.seat.get() {
            Some(a) => a,
            _ => {
                log::error!("Trying to create an offer from a unattached data source");
                return;
            }
        };
        let offer = ZwpPrimarySelectionOfferV1::create(client, self, &seat);
        let old = self.offer.set(offer);
        if let Some(offer) = old {
            offer.source.set(None);
        }
    }

    pub fn clear_offer(&self) {
        self.offer.take();
    }

    pub fn cancelled(self: &Rc<Self>) -> DynEventFormatter {
        Box::new(Cancelled { obj: self.clone() })
    }

    pub fn send(self: &Rc<Self>, mime_type: &str, fd: OwnedFd) -> DynEventFormatter {
        Box::new(Send {
            obj: self.clone(),
            mime_type: mime_type.to_string(),
            fd: Rc::new(fd),
        })
    }

    fn offer(&self, parser: MsgParser<'_, '_>) -> Result<(), OfferError> {
        let req: Offer = self.client.parse(self, parser)?;
        if self
            .mime_types
            .borrow_mut()
            .insert(req.mime_type.to_string())
        {
            if let Some(offer) = self.offer.get() {
                offer.client.event(offer.offer(req.mime_type));
            }
        }
        Ok(())
    }

    fn disconnect(&self) {
        if let Some(offer) = self.offer.take() {
            offer.source.set(None);
        }
        if let Some(seat) = self.seat.get() {
            let _ = seat.set_primary_selection(None);
        }
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.disconnect();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    ZwpPrimarySelectionSourceV1, ZwpPrimarySelectionSourceV1Error;

    OFFER => offer,
    DESTROY => destroy,
}

impl Object for ZwpPrimarySelectionSourceV1 {
    fn num_requests(&self) -> u32 {
        DESTROY + 1
    }

    fn break_loops(&self) {
        self.disconnect();
    }
}

dedicated_add_obj!(
    ZwpPrimarySelectionSourceV1,
    ZwpPrimarySelectionSourceV1Id,
    zwp_primary_selection_source
);
