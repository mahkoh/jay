
use crate::client::{Client, ClientError, DynEventFormatter};
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::ifs::zwp_primary_selection_offer_v1::ZwpPrimarySelectionOfferV1;
use crate::object::Object;
use crate::utils::buffd::{MsgParser, MsgParserError};
use crate::utils::clonecell::CloneCell;
use ahash::AHashSet;
use std::cell::RefCell;
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;
use crate::wire::zwp_primary_selection_source_v1::*;
use crate::wire::ZwpPrimarySelectionSourceV1Id;

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
        Box::new(Cancelled { self_id: self.id })
    }

    pub fn send(self: &Rc<Self>, mime_type: &str, fd: Rc<OwnedFd>) -> DynEventFormatter {
        Box::new(SendOut {
            self_id: self.id,
            mime_type: mime_type.to_string(),
            fd,
        })
    }

    fn offer(&self, parser: MsgParser<'_, '_>) -> Result<(), OfferError> {
        let req: OfferIn = self.client.parse(self, parser)?;
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

#[derive(Debug, Error)]
pub enum ZwpPrimarySelectionSourceV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process `offer` request")]
    OfferError(#[from] OfferError),
    #[error("Could not process `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("The data source is already attached")]
    AlreadyAttached,
}
efrom!(ZwpPrimarySelectionSourceV1Error, ClientError);

#[derive(Debug, Error)]
pub enum OfferError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(OfferError, ParseFailed, MsgParserError);
efrom!(OfferError, ClientError);

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ParseFailed, MsgParserError);
efrom!(DestroyError, ClientError);
