use crate::client::Client;
use crate::client::ClientError;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::object::Version;
use crate::rect::Rect;
use crate::rect::Region;
use crate::rect::RegionBuilder;
use crate::wire::WlRegionId;
use crate::wire::wl_region::*;
use std::cell::RefCell;
use std::rc::Rc;
use thiserror::Error;

pub struct WlRegion {
    id: WlRegionId,
    client: Rc<Client>,
    region: RefCell<RegionBuilder>,
    pub tracker: Tracker<Self>,
    version: Version,
}

impl WlRegion {
    pub fn new(id: WlRegionId, client: &Rc<Client>, version: Version) -> Self {
        Self {
            id,
            client: client.clone(),
            region: Default::default(),
            tracker: Default::default(),
            version,
        }
    }

    pub fn region(&self) -> Rc<Region> {
        self.region.borrow_mut().get()
    }
}

impl WlRegionRequestHandler for WlRegion {
    type Error = WlRegionError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn add(&self, req: Add, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if req.width < 0 || req.height < 0 {
            return Err(WlRegionError::NegativeExtents);
        }
        let mut region = self.region.borrow_mut();
        region.add(Rect::new_sized_saturating(
            req.x, req.y, req.width, req.height,
        ));
        Ok(())
    }

    fn subtract(&self, req: Subtract, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if req.width < 0 || req.height < 0 {
            return Err(WlRegionError::NegativeExtents);
        }
        let mut region = self.region.borrow_mut();
        region.sub(Rect::new_sized_saturating(
            req.x, req.y, req.width, req.height,
        ));
        Ok(())
    }
}

object_base! {
    self = WlRegion;
    version = self.version;
}

impl Object for WlRegion {}

dedicated_add_obj!(WlRegion, WlRegionId, regions);

#[derive(Debug, Error)]
pub enum WlRegionError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("width and/or height are negative")]
    NegativeExtents,
}
efrom!(WlRegionError, ClientError);
