mod types;

use crate::objects::{Interface, Object, ObjectError, ObjectId};
use crate::pixman::Region;
use crate::utils::buffd::{WlParser, WlParserError};
use crate::wl_client::{RequestParser, WlClientData};
use std::cell::RefCell;
use std::rc::Rc;
pub use types::*;
use crate::ifs::wl_display::WlDisplay;

const DESTROY: u32 = 0;
const ADD: u32 = 1;
const SUBTRACT: u32 = 2;

pub struct WlRegion {
    id: ObjectId,
    client: Rc<WlClientData>,
    rect: RefCell<Region>,
}

impl WlRegion {
    pub fn new(id: ObjectId, client: &Rc<WlClientData>) -> Self {
        Self {
            id,
            client: client.clone(),
            rect: RefCell::new(Region::new()),
        }
    }

    pub fn region(&self) -> Region {
        self.rect.borrow().clone()
    }

    async fn destroy(&self, parser: WlParser<'_, '_>) -> Result<(), DestroyError> {
        let _destroy: Destroy = self.client.parse(self, parser)?;
        self.client.objects.remove_obj(&self.client, self.id).await?;
        Ok(())
    }

    async fn add(&self, parser: WlParser<'_, '_>) -> Result<(), AddError> {
        let add: Add = self.client.parse(self, parser)?;
        if add.width < 0 || add.height < 0 {
            return Err(AddError::NegativeExtents);
        }
        let mut rect = self.rect.borrow_mut();
        *rect = rect.add(&Region::rect(add.x, add.y, add.width as _, add.height as _));
        Ok(())
    }

    async fn subtract(&self, parser: WlParser<'_, '_>) -> Result<(), SubtractError> {
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

    async fn handle_request_(
        &self,
        request: u32,
        parser: WlParser<'_, '_>,
    ) -> Result<(), WlRegionError> {
        match request {
            DESTROY => self.destroy(parser).await?,
            ADD => self.add(parser).await?,
            SUBTRACT => self.subtract(parser).await?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

handle_request!(WlRegion);

impl Object for WlRegion {
    fn id(&self) -> ObjectId {
        self.id
    }

    fn interface(&self) -> Interface {
        Interface::WlRegion
    }

    fn num_requests(&self) -> u32 {
        SUBTRACT + 1
    }

    fn post_attach(self: Rc<Self>) {
        self.client.objects.regions.set(self.id, self.clone());
    }

    fn pre_release(&self) -> Result<(), ObjectError> {
        self.client.objects.regions.remove(&self.id);
        Ok(())
    }
}
