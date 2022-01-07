mod types;
pub mod wl_keyboard;
pub mod wl_pointer;
pub mod wl_touch;

use crate::backend::{Seat, SeatEvent};
use crate::client::{AddObj, Client, ClientId, DynEventFormatter};
use crate::globals::{Global, GlobalName};
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use crate::utils::copyhashmap::CopyHashMap;
use std::rc::Rc;
pub use types::*;
use crate::ifs::wl_seat::wl_keyboard::WlKeyboard;
use crate::ifs::wl_seat::wl_pointer::WlPointer;
use crate::ifs::wl_seat::wl_touch::WlTouch;

id!(WlSeatId);

const GET_POINTER: u32 = 0;
const GET_KEYBOARD: u32 = 1;
const GET_TOUCH: u32 = 2;
const RELEASE: u32 = 3;

const CAPABILITIES: u32 = 0;
const NAME: u32 = 1;

const POINTER: u32 = 1;
const KEYBOARD: u32 = 2;
const TOUCH: u32 = 4;

const MISSING_CAPABILITY: u32 = 0;

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
        client.event(obj.capabilities()).await?;
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
    fn capabilities(self: &Rc<Self>) -> DynEventFormatter {
        Box::new(Capabilities {
            obj: self.clone(),
            capabilities: POINTER | KEYBOARD,
        })
    }

    async fn get_pointer(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), GetPointerError> {
        let req: GetPointer = self.client.parse(&**self, parser)?;
        let p = Rc::new(WlPointer::new(req.id, self));
        self.client.add_client_obj(&p)?;
        Ok(())
    }

    async fn get_keyboard(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), GetKeyboardError> {
        let req: GetKeyboard = self.client.parse(&**self, parser)?;
        let p = Rc::new(WlKeyboard::new(req.id, self));
        self.client.add_client_obj(&p)?;
        Ok(())
    }

    async fn get_touch(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), GetTouchError> {
        let req: GetTouch = self.client.parse(&**self, parser)?;
        let p = Rc::new(WlTouch::new(req.id, self));
        self.client.add_client_obj(&p)?;
        Ok(())
    }

    async fn release(&self, parser: MsgParser<'_, '_>) -> Result<(), ReleaseError> {
        let _req: Release = self.client.parse(self, parser)?;
        self.global.bindings.remove(&(self.client.id, self.id));
        self.client.remove_obj(self).await?;
        Ok(())
    }

    async fn handle_request_(
        self: &Rc<Self>,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlSeatError> {
        match request {
            GET_POINTER => self.get_pointer(parser).await?,
            GET_KEYBOARD => self.get_keyboard(parser).await?,
            GET_TOUCH => self.get_touch(parser).await?,
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
