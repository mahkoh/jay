mod types;

use crate::client::{Client, DynEventFormatter};
use crate::ifs::wl_data_offer::{DataOfferRole, WlDataOffer};
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use crate::utils::clonecell::{CloneCell, UnsafeCellCloneSafe};
use ahash::AHashSet;
use std::cell::RefCell;
use std::rc::Rc;
use uapi::OwnedFd;
pub use types::*;

const OFFER: u32 = 0;
const DESTROY: u32 = 1;
const SET_ACTIONS: u32 = 2;

const TARGET: u32 = 0;
const SEND: u32 = 1;
const CANCELLED: u32 = 2;
const DND_DROP_PERFORMED: u32 = 4;
const DND_FINISHED: u32 = 5;
const ACTION: u32 = 5;

#[allow(dead_code)]
const INVALID_ACTION_MASK: u32 = 0;
#[allow(dead_code)]
const INVALID_SOURCE: u32 = 1;

id!(WlDataSourceId);

#[derive(Clone)]
struct Attachment {
    seat: Rc<WlSeatGlobal>,
    role: DataOfferRole,
}

unsafe impl UnsafeCellCloneSafe for Attachment {}

pub struct WlDataSource {
    pub id: WlDataSourceId,
    pub client: Rc<Client>,
    pub mime_types: RefCell<AHashSet<String>>,
    attachment: CloneCell<Option<Attachment>>,
    offer: CloneCell<Option<Rc<WlDataOffer>>>,
}

impl WlDataSource {
    pub fn new(id: WlDataSourceId, client: &Rc<Client>) -> Self {
        Self {
            id,
            client: client.clone(),
            mime_types: RefCell::new(Default::default()),
            attachment: Default::default(),
            offer: Default::default(),
        }
    }

    pub fn attach(
        &self,
        seat: &Rc<WlSeatGlobal>,
        role: DataOfferRole,
    ) -> Result<(), WlDataSourceError> {
        let old = self.attachment.set(
            Some(Attachment {
                seat: seat.clone(),
                role,
            }),
        );
        if old.is_some() {
            return Err(WlDataSourceError::AlreadyAttached);
        }
        Ok(())
    }

    pub fn detach(self: &Rc<Self>) {
        self.attachment.set(None);
        if let Some(offer) = self.offer.set(None) {
            offer.source.set(None);
        }
        self.client.event(self.cancelled());
        self.client.flush();
    }

    pub fn create_offer(self: &Rc<Self>, client: &Rc<Client>) {
        let attachment = match self.attachment.get() {
            Some(a) => a,
            _ => {
                log::error!("Trying to create an offer from a unattached data source");
                return;
            }
        };
        let offer = WlDataOffer::create(client, attachment.role, self, &attachment.seat);
        let old = self.offer.set(offer);
        if let Some(offer) = old {
            offer.source.set(None);
        }
    }

    pub fn destroy_offer(&self) {
        self.offer.take();
    }

    pub fn cancelled(self: &Rc<Self>) -> DynEventFormatter {
        Box::new(Cancelled {
            obj: self.clone(),
        })
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
        if let Some(attachment) = self.attachment.get() {
            match attachment.role {
                DataOfferRole::Selection => {
                    let _ = attachment.seat.set_selection(None);
                },
            }
        }
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.disconnect();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_actions(&self, parser: MsgParser<'_, '_>) -> Result<(), SetActionsError> {
        let _req: SetActions = self.client.parse(self, parser)?;
        Ok(())
    }

    fn handle_request_(
        self: &Rc<Self>,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlDataSourceError> {
        match request {
            OFFER => self.offer(parser)?,
            DESTROY => self.destroy(parser)?,
            SET_ACTIONS => self.set_actions(parser)?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

handle_request!(WlDataSource);

impl Object for WlDataSource {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::WlDataSource
    }

    fn num_requests(&self) -> u32 {
        SET_ACTIONS + 1
    }

    fn break_loops(&self) {
        self.disconnect();
    }
}
