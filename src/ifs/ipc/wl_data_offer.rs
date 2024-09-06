use {
    crate::{
        client::{Client, ClientError, ClientId},
        fixed::Fixed,
        ifs::{
            ipc::{
                break_offer_loops, cancel_offer, destroy_data_offer, receive_data_offer,
                wl_data_device::{ClipboardIpc, WlDataDevice},
                wl_data_device_manager::DND_ALL,
                DataOffer, DataOfferId, DynDataOffer, OfferData, Role, OFFER_STATE_ACCEPTED,
                OFFER_STATE_DROPPED, OFFER_STATE_FINISHED, SOURCE_STATE_FINISHED,
            },
            wl_seat::WlSeatGlobal,
        },
        leaks::Tracker,
        object::Object,
        utils::bitflags::BitflagsExt,
        wire::{wl_data_offer::*, WlDataOfferId, WlSurfaceId},
    },
    std::rc::Rc,
    thiserror::Error,
};

#[expect(dead_code)]
const INVALID_FINISH: u32 = 0;
#[expect(dead_code)]
const INVALID_ACTION_MASK: u32 = 1;
#[expect(dead_code)]
const INVALID_ACTION: u32 = 2;
#[expect(dead_code)]
const INVALID_OFFER: u32 = 3;

pub struct WlDataOffer {
    pub id: WlDataOfferId,
    pub offer_id: DataOfferId,
    pub client: Rc<Client>,
    pub device: Rc<WlDataDevice>,
    pub data: OfferData<WlDataDevice>,
    pub tracker: Tracker<Self>,
}

impl DataOffer for WlDataOffer {
    type Device = WlDataDevice;

    fn offer_data(&self) -> &OfferData<Self::Device> {
        &self.data
    }
}

impl DynDataOffer for WlDataOffer {
    fn offer_id(&self) -> DataOfferId {
        self.offer_id
    }

    fn client_id(&self) -> ClientId {
        self.client.id
    }

    fn send_action(&self, action: u32) {
        WlDataOffer::send_action(self, action);
    }

    fn send_offer(&self, mime_type: &str) {
        WlDataOffer::send_offer(self, mime_type);
    }

    fn cancel(&self) {
        cancel_offer::<ClipboardIpc>(self);
    }

    fn send_enter(&self, surface: WlSurfaceId, x: Fixed, y: Fixed, serial: u32) {
        self.device.send_enter(surface, x, y, self.id, serial);
    }

    fn send_source_actions(&self) {
        WlDataOffer::send_source_actions(self);
    }

    fn get_seat(&self) -> Rc<WlSeatGlobal> {
        self.device.seat.clone()
    }
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
            if let Some(source_actions) = src.source_data().actions.get() {
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
}

impl WlDataOfferRequestHandler for WlDataOffer {
    type Error = WlDataOfferError;

    fn accept(&self, req: Accept, _slf: &Rc<Self>) -> Result<(), Self::Error> {
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

    fn receive(&self, req: Receive, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if self.data.shared.state.get().contains(OFFER_STATE_FINISHED) {
            return Err(WlDataOfferError::AlreadyFinished);
        }
        receive_data_offer::<ClipboardIpc>(self, req.mime_type, req.fd);
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        destroy_data_offer::<ClipboardIpc>(self);
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn finish(&self, _req: Finish, _slf: &Rc<Self>) -> Result<(), Self::Error> {
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
            src.source_data().state.or_assign(SOURCE_STATE_FINISHED);
            src.send_dnd_finished();
        } else {
            log::error!("no source");
        }
        self.data.shared.state.set(state);
        Ok(())
    }

    fn set_actions(&self, req: SetActions, _slf: &Rc<Self>) -> Result<(), Self::Error> {
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
    self = WlDataOffer;
    version = self.device.version;
}

impl Object for WlDataOffer {
    fn break_loops(&self) {
        break_offer_loops::<ClipboardIpc>(self);
    }
}

simple_add_obj!(WlDataOffer);

#[derive(Debug, Error)]
pub enum WlDataOfferError {
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
    #[error("The set of actions is invalid")]
    InvalidActions,
    #[error("Multiple preferred actions were specified")]
    MultiplePreferred,
}
efrom!(WlDataOfferError, ClientError);
