mod types;

use crate::client::{Client, DynEventFormatter};
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::ifs::zwp_primary_selection_source_v1::ZwpPrimarySelectionSourceV1;
use crate::object::Object;
use crate::utils::buffd::MsgParser;
use crate::utils::clonecell::CloneCell;
use std::ops::Deref;
use std::rc::Rc;
pub use types::*;

const RECEIVE: u32 = 0;
const DESTROY: u32 = 1;

const OFFER: u32 = 0;

id!(ZwpPrimarySelectionOfferV1Id);

pub struct ZwpPrimarySelectionOfferV1 {
    pub id: ZwpPrimarySelectionOfferV1Id,
    pub client: Rc<Client>,
    pub source: CloneCell<Option<Rc<ZwpPrimarySelectionSourceV1>>>,
}

impl ZwpPrimarySelectionOfferV1 {
    pub fn create(
        client: &Rc<Client>,
        src: &Rc<ZwpPrimarySelectionSourceV1>,
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
            source: CloneCell::new(Some(src.clone())),
        });
        let mt = src.mime_types.borrow_mut();
        seat.for_each_primary_selection_device(0, client.id, |device| {
            client.event(device.data_offer(slf.id));
            for mt in mt.deref() {
                client.event(slf.offer(mt));
            }
            client.event(device.selection(id));
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
            src.clear_offer();
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
    ZwpPrimarySelectionOfferV1, ZwpPrimarySelectionOfferV1Error;

    RECEIVE => receive,
    DESTROY => destroy,
}

impl Object for ZwpPrimarySelectionOfferV1 {
    fn num_requests(&self) -> u32 {
        DESTROY + 1
    }

    fn break_loops(&self) {
        self.disconnect();
    }
}

simple_add_obj!(ZwpPrimarySelectionOfferV1);
