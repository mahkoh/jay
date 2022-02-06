use crate::client::{Client, ClientError};
use crate::ifs::wl_data_offer::{DataOfferRole, WlDataOffer};
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::object::Object;
use crate::utils::buffd::MsgParser;
use crate::utils::buffd::MsgParserError;
use crate::utils::clonecell::{CloneCell, UnsafeCellCloneSafe};
use crate::wire::wl_data_source::*;
use crate::wire::WlDataSourceId;
use ahash::AHashSet;
use std::cell::RefCell;
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;

#[allow(dead_code)]
const INVALID_ACTION_MASK: u32 = 0;
#[allow(dead_code)]
const INVALID_SOURCE: u32 = 1;

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
        if self.attachment.get().is_some() {
            return Err(WlDataSourceError::AlreadyAttached);
        }
        self.attachment.set(Some(Attachment {
            seat: seat.clone(),
            role,
        }));
        Ok(())
    }

    pub fn detach(self: &Rc<Self>) {
        self.attachment.set(None);
        if let Some(offer) = self.offer.set(None) {
            offer.source.set(None);
        }
        self.send_cancelled();
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

    pub fn send_cancelled(self: &Rc<Self>) {
        self.client.event(Cancelled { self_id: self.id })
    }

    pub fn send_send(&self, mime_type: &str, fd: Rc<OwnedFd>) {
        self.client.event(Send {
            self_id: self.id,
            mime_type,
            fd,
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
                offer.send_offer(req.mime_type);
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
                }
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
}

object_base! {
    WlDataSource, WlDataSourceError;

    OFFER => offer,
    DESTROY => destroy,
    SET_ACTIONS => set_actions,
}

impl Object for WlDataSource {
    fn num_requests(&self) -> u32 {
        SET_ACTIONS + 1
    }

    fn break_loops(&self) {
        self.disconnect();
    }
}

dedicated_add_obj!(WlDataSource, WlDataSourceId, wl_data_source);

#[derive(Debug, Error)]
pub enum WlDataSourceError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process `offer` request")]
    OfferError(#[from] OfferError),
    #[error("Could not process `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process `set_actions` request")]
    SetActionsError(#[from] SetActionsError),
    #[error("The data source is already attached")]
    AlreadyAttached,
}
efrom!(WlDataSourceError, ClientError);

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

#[derive(Debug, Error)]
pub enum SetActionsError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(SetActionsError, ParseFailed, MsgParserError);
efrom!(SetActionsError, ClientError);
