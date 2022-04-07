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
        let mut region = self.region.borrow_mut();
        region.add(Rect::new_sized(add.x, add.y, add.width, add.height).unwrap());
        Ok(())
    }

    fn subtract(&self, parser: MsgParser<'_, '_>) -> Result<(), SubtractError> {
        let req: Subtract = self.client.parse(self, parser)?;
        if req.width < 0 || req.height < 0 {
            return Err(SubtractError::NegativeExtents);
        }
        let mut region = self.region.borrow_mut();
        region.sub(Rect::new_sized(req.x, req.y, req.width, req.height).unwrap());
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

#[derive(Debug, Error)]
pub enum WlRegionError {
    #[error("Could not process `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process `add` request")]
    AddError(#[from] AddError),
    #[error("Could not process `subtract` request")]
    SubtractError(#[from] SubtractError),
}

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ParseFailed, MsgParserError);
efrom!(DestroyError, ClientError);

#[derive(Debug, Error)]
pub enum AddError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error("width and/or height are negative")]
    NegativeExtents,
}
efrom!(AddError, ParseFailed, MsgParserError);

#[derive(Debug, Error)]
pub enum SubtractError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error("width and/or height are negative")]
    NegativeExtents,
}
efrom!(SubtractError, ParseFailed, MsgParserError);
