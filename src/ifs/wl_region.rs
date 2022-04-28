use {
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::Object,
        rect::{Rect, Region, RegionBuilder},
        utils::buffd::{MsgParser, MsgParserError},
        wire::{wl_region::*, WlRegionId},
    },
    std::{cell::RefCell, rc::Rc},
    thiserror::Error,
};

pub struct WlRegion {
    id: WlRegionId,
    client: Rc<Client>,
    region: RefCell<RegionBuilder>,
    pub tracker: Tracker<Self>,
}

impl WlRegion {
    pub fn new(id: WlRegionId, client: &Rc<Client>) -> Self {
        Self {
            id,
            client: client.clone(),
            region: Default::default(),
            tracker: Default::default(),
        }
    }

    pub fn region(&self) -> Rc<Region> {
        self.region.borrow_mut().get()
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), WlRegionError> {
        let _destroy: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn add(&self, parser: MsgParser<'_, '_>) -> Result<(), WlRegionError> {
        let add: Add = self.client.parse(self, parser)?;
        if add.width < 0 || add.height < 0 {
            return Err(WlRegionError::NegativeExtents);
        }
        let mut region = self.region.borrow_mut();
        region.add(Rect::new_sized(add.x, add.y, add.width, add.height).unwrap());
        Ok(())
    }

    fn subtract(&self, parser: MsgParser<'_, '_>) -> Result<(), WlRegionError> {
        let req: Subtract = self.client.parse(self, parser)?;
        if req.width < 0 || req.height < 0 {
            return Err(WlRegionError::NegativeExtents);
        }
        let mut region = self.region.borrow_mut();
        region.sub(Rect::new_sized(req.x, req.y, req.width, req.height).unwrap());
        Ok(())
    }
}

object_base! {
    WlRegion;

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

#[derive(Debug, Error)]
pub enum WlRegionError {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("width and/or height are negative")]
    NegativeExtents,
}
efrom!(WlRegionError, MsgParserError);
efrom!(WlRegionError, ClientError);
