use crate::client::{Client, ClientError};
use crate::ifs::ipc::wl_data_device::WlDataDevice;
use crate::ifs::ipc::{break_offer_loops, destroy_offer, receive, OfferData, Role, OFFER_STATE_FINISHED, OFFER_STATE_DROPPED, OFFER_STATE_ACCEPTED, SOURCE_STATE_FINISHED};
use crate::object::Object;
use crate::utils::buffd::MsgParser;
use crate::utils::buffd::MsgParserError;
use crate::wire::wl_data_offer::*;
use crate::wire::WlDataOfferId;
use std::rc::Rc;
use thiserror::Error;
use crate::ifs::ipc::wl_data_device_manager::{DND_ALL};
use crate::utils::bitflags::BitflagsExt;

#[allow(dead_code)]
const INVALID_FINISH: u32 = 0;
#[allow(dead_code)]
const INVALID_ACTION_MASK: u32 = 1;
#[allow(dead_code)]
const INVALID_ACTION: u32 = 2;
#[allow(dead_code)]
const INVALID_OFFER: u32 = 3;

pub struct WlDataOffer {
    pub id: WlDataOfferId,
    pub client: Rc<Client>,
    pub device: Rc<WlDataDevice>,
    pub data: OfferData<WlDataDevice>,
}

impl WlDataOffer {
    pub fn send_offer(&self, mime_type: &str) {
        self.client.event(Offer {
            self_id: self.id,
            mime_type,
        })
    }

    pub fn send_source_actions(&self) {
        if let Some(src) = self.data.source.get() {
            if let Some(source_actions) = src.data.actions.get() {
                self.client.event(SourceActions {
                    self_id: self.id,
                    source_actions,
                })
            }
        }
    }

    pub fn send_action(&self, dnd_action: u32) {
        self.client.event(Action {
            self_id: self.id,
            dnd_action,
        })
    }

    fn accept(&self, parser: MsgParser<'_, '_>) -> Result<(), AcceptError> {
        let req: Accept = self.client.parse(self, parser)?;
        let mut state = self.data.shared.state.get();
        if state.contains(OFFER_STATE_FINISHED) {
            return Err(AcceptError::AlreadyFinished);
        }
        if req.mime_type.is_some() {
            state |= OFFER_STATE_ACCEPTED;
        } else {
            state &= !OFFER_STATE_ACCEPTED;
        }
        self.data.shared.state.set(state);
        if let Some(src) = self.data.source.get() {
            src.send_target(req.mime_type);
        }
        Ok(())
    }

    fn receive(&self, parser: MsgParser<'_, '_>) -> Result<(), ReceiveError> {
        let req: Receive = self.client.parse(self, parser)?;
        if self.data.shared.state.get().contains(OFFER_STATE_FINISHED) {
            return Err(ReceiveError::AlreadyFinished);
        }
        receive::<WlDataDevice>(self, req.mime_type, req.fd);
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        destroy_offer::<WlDataDevice>(self);
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn finish(&self, parser: MsgParser<'_, '_>) -> Result<(), FinishError> {
        let _req: Finish = self.client.parse(self, parser)?;
        if self.data.shared.role.get() != Role::Dnd {
            return Err(FinishError::NotDnd);
        }
        let mut state = self.data.shared.state.get();
        if state.contains(OFFER_STATE_FINISHED) {
            return Err(FinishError::AlreadyFinished);
        }
        if !state.contains(OFFER_STATE_DROPPED) {
            return Err(FinishError::StillDragging);
        }
        if !state.contains(OFFER_STATE_ACCEPTED) {
            return Err(FinishError::NoMimeTypeAccepted);
        }
        state |= OFFER_STATE_FINISHED;
        if let Some(src) = self.data.source.get() {
            src.data.state.or_assign(SOURCE_STATE_FINISHED);
            src.send_dnd_finished();
        } else {
            log::error!("no source");
        }
        self.data.shared.state.set(state);
        Ok(())
    }

    fn set_actions(&self, parser: MsgParser<'_, '_>) -> Result<(), SetActionsError> {
        let req: SetActions = self.client.parse(self, parser)?;
        let state = self.data.shared.state.get();
        if state.contains(OFFER_STATE_FINISHED) {
            return Err(SetActionsError::AlreadyFinished);
        }
        if (req.dnd_actions & !DND_ALL, req.preferred_action & !DND_ALL) != (0, 0) {
            return Err(SetActionsError::InvalidActions);
        }
        if req.preferred_action.count_ones() > 1 {
            return Err(SetActionsError::MultiplePreferred);
        }
        self.data.shared.receiver_actions.set(req.dnd_actions);
        self.data.shared.receiver_preferred_action.set(req.preferred_action);
        if let Some(src) = self.data.source.get() {
            src.update_selected_action();
        }
        Ok(())
    }
}

object_base! {
    WlDataOffer, WlDataOfferError;

    ACCEPT => accept,
    RECEIVE => receive,
    DESTROY => destroy,
    FINISH => finish,
    SET_ACTIONS => set_actions,
}

impl Object for WlDataOffer {
    fn num_requests(&self) -> u32 {
        SET_ACTIONS + 1
    }

    fn break_loops(&self) {
        break_offer_loops::<WlDataDevice>(self);
    }
}

simple_add_obj!(WlDataOffer);

#[derive(Debug, Error)]
pub enum WlDataOfferError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process `accept` request")]
    AcceptError(#[from] AcceptError),
    #[error("Could not process `receive` request")]
    ReceiveError(#[from] ReceiveError),
    #[error("Could not process `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process `finish` request")]
    FinishError(#[from] FinishError),
    #[error("Could not process `set_actions` request")]
    SetActionsError(#[from] SetActionsError),
}
efrom!(WlDataOfferError, ClientError);

#[derive(Debug, Error)]
pub enum AcceptError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("`finish` was already called")]
    AlreadyFinished,
}
efrom!(AcceptError, ParseFailed, MsgParserError);
efrom!(AcceptError, ClientError);

#[derive(Debug, Error)]
pub enum ReceiveError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("`finish` was already called")]
    AlreadyFinished,
}
efrom!(ReceiveError, ParseFailed, MsgParserError);
efrom!(ReceiveError, ClientError);

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
pub enum FinishError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("`finish` was already called")]
    AlreadyFinished,
    #[error("The drag operation is still ongoing")]
    StillDragging,
    #[error("Client did not accept a mime type")]
    NoMimeTypeAccepted,
    #[error("This is not a drag-and-drop offer")]
    NotDnd,
}
efrom!(FinishError, ParseFailed, MsgParserError);
efrom!(FinishError, ClientError);

#[derive(Debug, Error)]
pub enum SetActionsError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("`finish` was already called")]
    AlreadyFinished,
    #[error("The set of actions is invalid")]
    InvalidActions,
    #[error("Multiple preferred actions were specified")]
    MultiplePreferred,
}
efrom!(SetActionsError, ParseFailed, MsgParserError);
efrom!(SetActionsError, ClientError);
