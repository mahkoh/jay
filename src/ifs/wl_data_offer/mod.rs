mod types;

use crate::client::{Client, DynEventFormatter};
use crate::ifs::wl_data_source::{WlDataSource};
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use crate::utils::clonecell::CloneCell;
use std::ops::Deref;
use std::rc::Rc;
pub use types::*;

const ACCEPT: u32 = 0;
const RECEIVE: u32 = 1;
const DESTROY: u32 = 2;
const FINISH: u32 = 3;
const SET_ACTIONS: u32 = 4;

const OFFER: u32 = 0;
const SOURCE_ACTIONS: u32 = 1;
const ACTION: u32 = 2;

#[allow(dead_code)]
const INVALID_FINISH: u32 = 0;
#[allow(dead_code)]
const INVALID_ACTION_MASK: u32 = 1;
#[allow(dead_code)]
const INVALID_ACTION: u32 = 2;
#[allow(dead_code)]
const INVALID_OFFER: u32 = 3;

id!(WlDataOfferId);

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum DataOfferRole {
    Selection,
}

pub struct WlDataOffer {
    pub id: WlDataOfferId,
    pub client: Rc<Client>,
    pub role: DataOfferRole,
    pub source: CloneCell<Option<Rc<WlDataSource>>>,
}

impl WlDataOffer {
    pub fn create(
        client: &Rc<Client>,
        role: DataOfferRole,
        src: &Rc<WlDataSource>,
        seat: &Rc<WlSeatGlobal>,
    ) -> Option<Rc<Self>> {
        let id = match client.new_id() {
            Ok(id) => id,
            Err(e) => {
                client.error(e);
                return None;
            }
        };
        let slf = Rc::new(Self {
            id,
            client: client.clone(),
            role,
            source: CloneCell::new(Some(src.clone())),
        });
        let mt = src.mime_types.borrow_mut();
        seat.for_each_data_device(0, client.id, |device| {
            client.event(device.data_offer(slf.id));
            for mt in mt.deref() {
                client.event(slf.offer(mt));
            }
            let ev = match role {
                DataOfferRole::Selection => device.selection(id),
            };
            client.event(ev);
        });
        client.add_server_obj(&slf);
        Some(slf)
    }

    pub fn offer(self: &Rc<Self>, mime_type: &str) -> DynEventFormatter {
        Box::new(Offer {
            obj: self.clone(),
            mime_type: mime_type.to_string(),
        })
    }

    fn accept(&self, parser: MsgParser<'_, '_>) -> Result<(), AcceptError> {
        let _req: Accept = self.client.parse(self, parser)?;
        Ok(())
    }

    fn receive(&self, parser: MsgParser<'_, '_>) -> Result<(), ReceiveError> {
        let req: Receive = self.client.parse(self, parser)?;
        if let Some(src) = self.source.get() {
            src.client.event(src.send(req.mime_type, req.fd));
            src.client.flush();
        }
        Ok(())
    }

    fn disconnect(&self) {
        if let Some(src) = self.source.set(None) {
            src.destroy_offer();
        }
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.disconnect();
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn finish(&self, parser: MsgParser<'_, '_>) -> Result<(), FinishError> {
        let _req: Finish = self.client.parse(self, parser)?;
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
    ) -> Result<(), WlDataOfferError> {
        match request {
            ACCEPT => self.accept(parser)?,
            RECEIVE => self.receive(parser)?,
            DESTROY => self.destroy(parser)?,
            FINISH => self.finish(parser)?,
            SET_ACTIONS => self.set_actions(parser)?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

handle_request!(WlDataOffer);

impl Object for WlDataOffer {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::WlDataOffer
    }

    fn num_requests(&self) -> u32 {
        SET_ACTIONS + 1
    }

    fn break_loops(&self) {
        self.disconnect();
    }
}
