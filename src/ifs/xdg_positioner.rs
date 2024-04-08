use {
    crate::{
        client::{Client, ClientError},
        ifs::xdg_wm_base::XdgWmBase,
        leaks::Tracker,
        object::Object,
        rect::Rect,
        wire::{xdg_positioner::*, XdgPositionerId},
    },
    std::{cell::RefCell, rc::Rc},
    thiserror::Error,
};

const INVALID_INPUT: u32 = 0;

const NONE: u32 = 0;
const TOP: u32 = 1;
const BOTTOM: u32 = 2;
const LEFT: u32 = 3;
const RIGHT: u32 = 4;
const TOP_LEFT: u32 = 5;
const BOTTOM_LEFT: u32 = 6;
const TOP_RIGHT: u32 = 7;
const BOTTOM_RIGHT: u32 = 8;

bitflags! {
    Edge: u32;
        E_TOP = 1 << 0,
        E_BOTTOM = 1 << 1,
        E_LEFT = 1 << 2,
        E_RIGHT = 1 << 3,
}

impl Edge {
    fn from_enum(e: u32) -> Option<Self> {
        let s = match e {
            NONE => Self::none(),
            TOP => E_TOP,
            BOTTOM => E_BOTTOM,
            LEFT => E_LEFT,
            RIGHT => E_RIGHT,
            TOP_LEFT => E_TOP | E_LEFT,
            BOTTOM_LEFT => E_BOTTOM | E_LEFT,
            TOP_RIGHT => E_TOP | E_RIGHT,
            BOTTOM_RIGHT => E_BOTTOM | E_RIGHT,
            _ => return None,
        };
        Some(s)
    }
}

bitflags! {
    CA: u32;
        CA_NONE = 0,
        CA_SLIDE_X = 1,
        CA_SLIDE_Y = 2,
        CA_FLIP_X = 4,
        CA_FLIP_Y = 8,
        CA_RESIZE_X = 16,
        CA_RESIZE_Y = 32,
}

pub struct XdgPositioner {
    id: XdgPositionerId,
    base: Rc<XdgWmBase>,
    client: Rc<Client>,
    position: RefCell<XdgPositioned>,
    pub tracker: Tracker<Self>,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct XdgPositioned {
    pub size_width: i32,
    pub size_height: i32,
    pub ar: Rect,
    pub anchor: Edge,
    pub gravity: Edge,
    pub ca: CA,
    pub off_x: i32,
    pub off_y: i32,
    pub reactive: bool,
    pub parent_width: i32,
    pub parent_height: i32,
    pub parent_serial: u32,
}

impl XdgPositioned {
    pub fn is_complete(&self) -> bool {
        self.size_height != 0 && self.size_width != 0
    }

    pub fn get_position(&self, flip_x: bool, flip_y: bool) -> Rect {
        let mut anchor = self.anchor;
        let mut gravity = self.gravity;
        if flip_x {
            anchor ^= E_LEFT | E_RIGHT;
            gravity ^= E_LEFT | E_RIGHT;
        }
        if flip_y {
            anchor ^= E_TOP | E_BOTTOM;
            gravity ^= E_TOP | E_BOTTOM;
        }

        let mut x1 = self.off_x;
        let mut y1 = self.off_x;

        if anchor.contains(E_LEFT) {
            x1 += self.ar.x1();
        } else if anchor.contains(E_RIGHT) {
            x1 += self.ar.x2();
        } else {
            x1 += self.ar.x1() + self.ar.width() / 2;
        }

        if anchor.contains(E_TOP) {
            y1 += self.ar.y1();
        } else if anchor.contains(E_BOTTOM) {
            y1 += self.ar.y2();
        } else {
            y1 += self.ar.y1() + self.ar.height() / 2;
        }

        if gravity.contains(E_LEFT) {
            x1 -= self.size_width;
        } else if !gravity.contains(E_RIGHT) {
            x1 -= self.size_width / 2;
        }

        if gravity.contains(E_TOP) {
            y1 -= self.size_height;
        } else if !gravity.contains(E_BOTTOM) {
            y1 -= self.size_height / 2;
        }

        Rect::new_sized(x1, y1, self.size_width, self.size_height).unwrap()
    }
}

impl XdgPositioner {
    pub fn new(base: &Rc<XdgWmBase>, id: XdgPositionerId, client: &Rc<Client>) -> Self {
        Self {
            id,
            client: client.clone(),
            base: base.clone(),
            position: RefCell::new(Default::default()),
            tracker: Default::default(),
        }
    }

    pub fn value(&self) -> XdgPositioned {
        *self.position.borrow()
    }
}

impl XdgPositionerRequestHandler for XdgPositioner {
    type Error = XdgPositionerError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_size(&self, req: SetSize, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if req.width <= 0 || req.height <= 0 {
            self.client.protocol_error(
                self,
                INVALID_INPUT,
                &format!("Cannot set a non-positive size"),
            );
            return Err(XdgPositionerError::NonPositiveSize);
        }
        let mut position = self.position.borrow_mut();
        position.size_width = req.width;
        position.size_height = req.height;
        Ok(())
    }

    fn set_anchor_rect(&self, req: SetAnchorRect, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if req.width < 0 || req.height < 0 {
            self.client.protocol_error(
                self,
                INVALID_INPUT,
                &format!("Cannot set an anchor rect with negative size"),
            );
            return Err(XdgPositionerError::NegativeAnchorRect);
        }
        let mut position = self.position.borrow_mut();
        position.ar = Rect::new_sized(req.x, req.y, req.width, req.height).unwrap();
        Ok(())
    }

    fn set_anchor(&self, req: SetAnchor, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let anchor = match Edge::from_enum(req.anchor) {
            Some(a) => a,
            _ => return Err(XdgPositionerError::UnknownAnchor(req.anchor)),
        };
        self.position.borrow_mut().anchor = anchor;
        Ok(())
    }

    fn set_gravity(&self, req: SetGravity, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let gravity = match Edge::from_enum(req.gravity) {
            Some(a) => a,
            _ => return Err(XdgPositionerError::UnknownGravity(req.gravity)),
        };
        self.position.borrow_mut().gravity = gravity;
        Ok(())
    }

    fn set_constraint_adjustment(
        &self,
        req: SetConstraintAdjustment,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let ca = CA(req.constraint_adjustment);
        if !ca.is_valid() {
            return Err(XdgPositionerError::UnknownCa(req.constraint_adjustment));
        }
        self.position.borrow_mut().ca = ca;
        Ok(())
    }

    fn set_offset(&self, req: SetOffset, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let mut position = self.position.borrow_mut();
        position.off_x = req.x;
        position.off_y = req.y;
        Ok(())
    }

    fn set_reactive(&self, _req: SetReactive, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.position.borrow_mut().reactive = true;
        Ok(())
    }

    fn set_parent_size(&self, req: SetParentSize, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if req.parent_width < 0 || req.parent_height < 0 {
            self.client.protocol_error(
                self,
                INVALID_INPUT,
                &format!("Cannot set a negative parent size"),
            );
            return Err(XdgPositionerError::NegativeParentSize);
        }
        let mut position = self.position.borrow_mut();
        position.parent_width = req.parent_width;
        position.parent_height = req.parent_height;
        Ok(())
    }

    fn set_parent_configure(
        &self,
        req: SetParentConfigure,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        self.position.borrow_mut().parent_serial = req.serial;
        Ok(())
    }
}

object_base! {
    self = XdgPositioner;
    version = self.base.version;
}

impl Object for XdgPositioner {}

dedicated_add_obj!(XdgPositioner, XdgPositionerId, xdg_positioners);

#[derive(Debug, Error)]
pub enum XdgPositionerError {
    #[error("Cannot set a non-positive size")]
    NonPositiveSize,
    #[error("Cannot set an anchor rect with a negative size")]
    NegativeAnchorRect,
    #[error("Unknown anchor {0}")]
    UnknownAnchor(u32),
    #[error("Unknown gravity {0}")]
    UnknownGravity(u32),
    #[error("Unknown constraint adjustment {0}")]
    UnknownCa(u32),
    #[error("Cannot set a negative parent size")]
    NegativeParentSize,
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(XdgPositionerError, ClientError);
