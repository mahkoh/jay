mod types;

use crate::client::Client;
use crate::object::Object;
use crate::pixman::Region;
use crate::utils::buffd::MsgParser;
use std::cell::RefCell;
use std::rc::Rc;
pub use types::*;

const DESTROY: u32 = 0;
const ADD: u32 = 1;
const SUBTRACT: u32 = 2;

id!(WlRegionId);

pub struct WlRegion {
    id: WlRegionId,
    client: Rc<Client>,
    rect: RefCell<Region>,
}

impl WlRegion {
    pub fn new(id: WlRegionId, client: &Rc<Client>) -> Self {
        Self {
            id,
            client: client.clone(),
            rect: RefCell::new(Region::new()),
        }
    }

    pub fn region(&self) -> Region {
        self.rect.borrow().clone()
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _destroy: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn add(&self, parser: MsgParser<'_, '_>) -> Result<(), AddError> {
        let add: Add = self.client.parse(self, parser)?;
        if add.width < 0 || add.height < 0 {
            return Err(AddError::NegativeExtents);
        }
        let mut rect = self.rect.borrow_mut();
        *rect = rect.add(&Region::rect(add.x, add.y, add.width as _, add.height as _));
        Ok(())
    }

    fn subtract(&self, parser: MsgParser<'_, '_>) -> Result<(), SubtractError> {
        let subtract: Subtract = self.client.parse(self, parser)?;
        if subtract.width < 0 || subtract.height < 0 {
            return Err(SubtractError::NegativeExtents);
        }
        let mut rect = self.rect.borrow_mut();
        *rect = rect.subtract(&Region::rect(
            subtract.x,
            subtract.y,
            subtract.width as _,
            subtract.height as _,
        ));
        Ok(())
    }
}

object_base! {
    WlRegion, WlRegionError;

    DESTROY => destroy,
    ADD => add,
    SUBTRACT => subtract,
}

impl Object for WlRegion {
    fn num_requests(&self) -> u32 {
        SUBTRACT + 1
    }
}

dedicated_add_obj!(WlRegion, WlRegionId, regions);
