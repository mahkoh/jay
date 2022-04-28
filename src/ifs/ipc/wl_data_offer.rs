use {
    crate::{
        client::{Client, ClientError},
        ifs::ipc::{
            break_offer_loops, destroy_offer, receive, wl_data_device::WlDataDevice,
            wl_data_device_manager::DND_ALL, OfferData, Role, OFFER_STATE_ACCEPTED,
            OFFER_STATE_DROPPED, OFFER_STATE_FINISHED, SOURCE_STATE_FINISHED,
        },
        leaks::Tracker,
        object::Object,
        utils::{
            bitflags::BitflagsExt,
            buffd::{MsgParser, MsgParserError},
        },
        wire::{wl_data_offer::*, WlDataOfferId},
    },
    std::rc::Rc,
    thiserror::Error,
};

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
    pub tracker: Tracker<Self>,
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

    fn accept(&self, parser: MsgParser<'_, '_>) -> Result<(), WlDataOfferError> {
        let req: Accept = self.client.parse(self, parser)?;
        let _ = req.serial; // unused
        let mut state = self.data.shared.state.get();
        if state.contains(OFFER_STATE_FINISHED) {
            return Err(WlDataOfferError::AlreadyFinished);
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

    fn receive(&self, parser: MsgParser<'_, '_>) -> Result<(), WlDataOfferError> {
        let req: Receive = self.client.parse(self, parser)?;
        if self.data.shared.state.get().contains(OFFER_STATE_FINISHED) {
            return Err(WlDataOfferError::AlreadyFinished);
        }
        receive::<WlDataDevice>(self, req.mime_type, req.fd);
        Ok(())
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), WlDataOfferError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        destroy_offer::<WlDataDevice>(self);
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn finish(&self, parser: MsgParser<'_, '_>) -> Result<(), WlDataOfferError> {
        let _req: Finish = self.client.parse(self, parser)?;
        if self.data.shared.role.get() != Role::Dnd {
            return Err(WlDataOfferError::NotDnd);
        }
        let mut state = self.data.shared.state.get();
        if state.contains(OFFER_STATE_FINISHED) {
            return Err(WlDataOfferError::AlreadyFinished);
        }
        if !state.contains(OFFER_STATE_DROPPED) {
            return Err(WlDataOfferError::StillDragging);
        }
        if !state.contains(OFFER_STATE_ACCEPTED) {
            return Err(WlDataOfferError::NoMimeTypeAccepted);
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

    fn set_actions(&self, parser: MsgParser<'_, '_>) -> Result<(), WlDataOfferError> {
        let req: SetActions = self.client.parse(self, parser)?;
        let state = self.data.shared.state.get();
        if state.contains(OFFER_STATE_FINISHED) {
            return Err(WlDataOfferError::AlreadyFinished);
        }
        if (req.dnd_actions & !DND_ALL, req.preferred_action & !DND_ALL) != (0, 0) {
            return Err(WlDataOfferError::InvalidActions);
        }
        if req.preferred_action.count_ones() > 1 {
            return Err(WlDataOfferError::MultiplePreferred);
        }
        self.data.shared.receiver_actions.set(req.dnd_actions);
        self.data
            .shared
            .receiver_preferred_action
            .set(req.preferred_action);
        if let Some(src) = self.data.source.get() {
            src.update_selected_action();
        }
        Ok(())
    }
}

object_base! {
    WlDataOffer;

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
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error("`finish` was already called")]
    AlreadyFinished,
    #[error("The drag operation is still ongoing")]
    StillDragging,
    #[error("Client did not accept a mime type")]
    NoMimeTypeAccepted,
    #[error("This is not a drag-and-drop offer")]
    NotDnd,
    #[error("The set of actions is invalid")]
    InvalidActions,
    #[error("Multiple preferred actions were specified")]
    MultiplePreferred,
}
efrom!(WlDataOfferError, ClientError);
efrom!(WlDataOfferError, MsgParserError);
