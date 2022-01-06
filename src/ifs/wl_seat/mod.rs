mod types;

use crate::backend::{Seat, SeatEvent};
use crate::client::{AddObj, Client, ClientId};
use crate::globals::{Global, GlobalName};
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use crate::utils::copyhashmap::CopyHashMap;
use std::rc::Rc;
pub use types::*;

id!(WlSeatId);

const RELEASE: u32 = 0;

pub struct WlSeatGlobal {
    name: GlobalName,
    seat: Rc<dyn Seat>,
    bindings: CopyHashMap<(ClientId, WlSeatId), Rc<WlSeatObj>>,
}

impl WlSeatGlobal {
    pub fn new(name: GlobalName, seat: &Rc<dyn Seat>) -> Self {
        Self {
            name,
            seat: seat.clone(),
            bindings: Default::default(),
        }
    }

    pub async fn event(&self, event: SeatEvent) {
        log::debug!("se: {:?}", event);
    }

    async fn bind_(
        self: Rc<Self>,
        id: WlSeatId,
        client: &Rc<Client>,
        version: u32,
    ) -> Result<(), WlSeatError> {
        let obj = Rc::new(WlSeatObj {
            global: self.clone(),
            id,
            client: client.clone(),
        });
        client.add_client_obj(&obj)?;
        self.bindings.set((client.id, id), obj.clone());
        Ok(())
    }
}

bind!(WlSeatGlobal);

impl Global for WlSeatGlobal {
    fn name(&self) -> GlobalName {
        self.name
    }

    fn interface(&self) -> Interface {
        Interface::WlSeat
    }

    fn version(&self) -> u32 {
        3
    }

    fn pre_remove(&self) {
        //
    }

    fn break_loops(&self) {
        self.bindings.clear();
    }
}

pub struct WlSeatObj {
    global: Rc<WlSeatGlobal>,
    id: WlSeatId,
    client: Rc<Client>,
}

impl WlSeatObj {
    async fn release(&self, parser: MsgParser<'_, '_>) -> Result<(), ReleaseError> {
        let _req: Release = self.client.parse(self, parser)?;
        self.global.bindings.remove(&(self.client.id, self.id));
        self.client.remove_obj(self).await?;
        Ok(())
    }

    async fn handle_request_(
        &self,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlSeatError> {
        match request {
            RELEASE => self.release(parser).await?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

handle_request!(WlSeatObj);

impl Object for WlSeatObj {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::WlSeat
    }

    fn num_requests(&self) -> u32 {
        RELEASE + 1
    }

    fn break_loops(&self) {
        self.global.bindings.remove(&(self.client.id, self.id));
    }
}
