mod types;

use crate::client::{AddObj, Client};
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use bitflags::bitflags;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use std::cell::RefCell;
use std::rc::Rc;
pub use types::*;

const DESTROY: u32 = 0;
const SET_SIZE: u32 = 1;
const SET_ANCHOR_RECT: u32 = 2;
const SET_ANCHOR: u32 = 3;
const SET_GRAVITY: u32 = 4;
const SET_CONSTRAINT_ADJUSTMENT: u32 = 5;
const SET_OFFSET: u32 = 6;
const SET_REACTIVE: u32 = 7;
const SET_PARENT_SIZE: u32 = 8;
const SET_PARENT_CONFIGURE: u32 = 9;

const INVALID_INPUT: u32 = 0;

#[derive(Debug, Eq, PartialEq, Copy, Clone, FromPrimitive)]
pub enum Anchor {
    None = 0,
    Top = 1,
    Bottom = 2,
    Left = 3,
    Right = 4,
    TopLeft = 5,
    BottomLeft = 6,
    TopRight = 7,
    BottomRight = 8,
}

impl Default for Anchor {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone, FromPrimitive)]
pub enum Gravity {
    None = 0,
    Top = 1,
    Bottom = 2,
    Left = 3,
    Right = 4,
    TopLeft = 5,
    BottomLeft = 6,
    TopRight = 7,
    BottomRight = 8,
}

impl Default for Gravity {
    fn default() -> Self {
        Self::None
    }
}

bitflags! {
    #[derive(Default)]
    pub struct CA: u32 {
        const NONE = 0;
        const SLIDE_X = 1;
        const SLIDE_Y = 2;
        const FLIP_X = 4;
        const FLIP_Y = 8;
        const RESIZE_X = 16;
        const RESIZE_Y = 32;
    }
}

id!(XdgPositionerId);

pub struct XdgPositioner {
    id: XdgPositionerId,
    client: Rc<Client>,
    version: u32,
    position: RefCell<XdgPositioned>,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct XdgPositioned {
    pub size_width: u32,
    pub size_height: u32,
    pub ar_x: i32,
    pub ar_y: i32,
    pub ar_width: u32,
    pub ar_height: u32,
    pub anchor: Anchor,
    pub gravity: Gravity,
    pub ca: CA,
    pub off_x: i32,
    pub off_y: i32,
    pub reactive: bool,
    pub parent_width: u32,
    pub parent_height: u32,
    pub parent_serial: u32,
}

impl XdgPositioner {
    pub fn new(id: XdgPositionerId, client: &Rc<Client>, version: u32) -> Self {
        Self {
            id,
            client: client.clone(),
            version,
            position: RefCell::new(Default::default()),
        }
    }

    pub fn clone(&self) -> Box<XdgPositioned> {
        Box::new(*self.position.borrow())
    }

    async fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self).await?;
        Ok(())
    }

    async fn set_size(&self, parser: MsgParser<'_, '_>) -> Result<(), SetSizeError> {
        let req: SetSize = self.client.parse(self, parser)?;
        if req.width <= 0 || req.height <= 0 {
            self.client.protocol_error(
                self,
                INVALID_INPUT,
                format!("Cannot set a non-positive size"),
            );
            return Err(SetSizeError::NonPositiveSize);
        }
        let mut position = self.position.borrow_mut();
        position.size_width = req.width as u32;
        position.size_height = req.height as u32;
        Ok(())
    }

    async fn set_anchor_rect(&self, parser: MsgParser<'_, '_>) -> Result<(), SetAnchorRectError> {
        let req: SetAnchorRect = self.client.parse(self, parser)?;
        if req.width < 0 || req.height < 0 {
            self.client.protocol_error(
                self,
                INVALID_INPUT,
                format!("Cannot set an anchor rect with negative size"),
            );
            return Err(SetAnchorRectError::NegativeAnchorRect);
        }
        let mut position = self.position.borrow_mut();
        position.ar_x = req.x;
        position.ar_y = req.y;
        position.ar_width = req.width as u32;
        position.ar_height = req.height as u32;
        Ok(())
    }

    async fn set_anchor(&self, parser: MsgParser<'_, '_>) -> Result<(), SetAnchorError> {
        let req: SetAnchor = self.client.parse(self, parser)?;
        let anchor = match Anchor::from_u32(req.anchor) {
            Some(a) => a,
            _ => return Err(SetAnchorError::UnknownAnchor(req.anchor)),
        };
        self.position.borrow_mut().anchor = anchor;
        Ok(())
    }

    async fn set_gravity(&self, parser: MsgParser<'_, '_>) -> Result<(), SetGravityError> {
        let req: SetGravity = self.client.parse(self, parser)?;
        let gravity = match Gravity::from_u32(req.gravity) {
            Some(a) => a,
            _ => return Err(SetGravityError::UnknownGravity(req.gravity)),
        };
        self.position.borrow_mut().gravity = gravity;
        Ok(())
    }

    async fn set_constraint_adjustment(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), SetConstraintAdjustmentError> {
        let req: SetConstraintAdjustment = self.client.parse(self, parser)?;
        let ca = match CA::from_bits(req.constraint_adjustment) {
            Some(c) => c,
            _ => {
                return Err(SetConstraintAdjustmentError::UnknownCa(
                    req.constraint_adjustment,
                ))
            }
        };
        self.position.borrow_mut().ca = ca;
        Ok(())
    }

    async fn set_offset(&self, parser: MsgParser<'_, '_>) -> Result<(), SetOffsetError> {
        let req: SetOffset = self.client.parse(self, parser)?;
        let mut position = self.position.borrow_mut();
        position.off_x = req.x;
        position.off_y = req.y;
        Ok(())
    }

    async fn set_reactive(&self, parser: MsgParser<'_, '_>) -> Result<(), SetReactiveError> {
        let _req: SetReactive = self.client.parse(self, parser)?;
        self.position.borrow_mut().reactive = true;
        Ok(())
    }

    async fn set_parent_size(&self, parser: MsgParser<'_, '_>) -> Result<(), SetParentSizeError> {
        let req: SetParentSize = self.client.parse(self, parser)?;
        if req.parent_width < 0 || req.parent_height < 0 {
            self.client.protocol_error(
                self,
                INVALID_INPUT,
                format!("Cannot set a negative parent size"),
            );
            return Err(SetParentSizeError::NegativeParentSize);
        }
        let mut position = self.position.borrow_mut();
        position.parent_width = req.parent_width as u32;
        position.parent_height = req.parent_height as u32;
        Ok(())
    }

    async fn set_parent_configure(
        &self,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), SetParentConfigureError> {
        let req: SetParentConfigure = self.client.parse(self, parser)?;
        self.position.borrow_mut().parent_serial = req.serial;
        Ok(())
    }

    async fn handle_request_(
        &self,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), XdgPositionerError> {
        match request {
            DESTROY => self.destroy(parser).await?,
            SET_SIZE => self.set_size(parser).await?,
            SET_ANCHOR_RECT => self.set_anchor_rect(parser).await?,
            SET_ANCHOR => self.set_anchor(parser).await?,
            SET_GRAVITY => self.set_gravity(parser).await?,
            SET_CONSTRAINT_ADJUSTMENT => self.set_constraint_adjustment(parser).await?,
            SET_OFFSET => self.set_offset(parser).await?,
            SET_REACTIVE => self.set_reactive(parser).await?,
            SET_PARENT_SIZE => self.set_parent_size(parser).await?,
            SET_PARENT_CONFIGURE => self.set_parent_configure(parser).await?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

handle_request!(XdgPositioner);

impl Object for XdgPositioner {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::XdgPositioner
    }

    fn num_requests(&self) -> u32 {
        SET_PARENT_CONFIGURE + 1
    }
}
