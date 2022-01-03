use crate::client::{ClientError, RequestParser};
use crate::utils::buffd::{MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum XdgPositionerError {
    #[error("Could not process a `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process a `set_size` request")]
    SetSizeError(#[from] SetSizeError),
    #[error("Could not process a `set_anchor_rect` request")]
    SetAnchorRectError(#[from] SetAnchorRectError),
    #[error("Could not process a `set_anchor` request")]
    SetAnchorError(#[from] SetAnchorError),
    #[error("Could not process a `set_gravity` request")]
    SetGravityError(#[from] SetGravityError),
    #[error("Could not process a `set_constraint_adjustment` request")]
    SetConstraintAdjustmentError(#[from] SetConstraintAdjustmentError),
    #[error("Could not process a `set_offset` request")]
    SetOffsetError(#[from] SetOffsetError),
    #[error("Could not process a `set_reactive` request")]
    SetReactiveError(#[from] SetReactiveError),
    #[error("Could not process a `set_parent_size` request")]
    SetParentSizeError(#[from] SetParentSizeError),
    #[error("Could not process a `set_parent_configure` request")]
    SetParentConfigureError(#[from] SetParentConfigureError),
}

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ParseError, MsgParserError);
efrom!(DestroyError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum SetSizeError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error("Cannot set a non-positive size")]
    NonPositiveSize,
}
efrom!(SetSizeError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum SetAnchorRectError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error("Cannot set an anchor rect with a negative size")]
    NegativeAnchorRect,
}
efrom!(SetAnchorRectError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum SetAnchorError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error("Unknown anchor {0}")]
    UnknownAnchor(u32),
}
efrom!(SetAnchorError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum SetGravityError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error("Unknown gravity {0}")]
    UnknownGravity(u32),
}
efrom!(SetGravityError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum SetConstraintAdjustmentError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error("Unknown constraint adjustment {0}")]
    UnknownCa(u32),
}
efrom!(SetConstraintAdjustmentError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum SetOffsetError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
}
efrom!(SetOffsetError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum SetReactiveError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
}
efrom!(SetReactiveError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum SetParentSizeError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error("Cannot set a negative parent size")]
    NegativeParentSize,
}
efrom!(SetParentSizeError, ParseError, MsgParserError);

#[derive(Debug, Error)]
pub enum SetParentConfigureError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
}
efrom!(SetParentConfigureError, ParseError, MsgParserError);

pub(super) struct Destroy;
impl RequestParser<'_> for Destroy {
    fn parse(_parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self)
    }
}
impl Debug for Destroy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "destroy()")
    }
}

pub(super) struct SetSize {
    pub width: i32,
    pub height: i32,
}
impl RequestParser<'_> for SetSize {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            width: parser.int()?,
            height: parser.int()?,
        })
    }
}
impl Debug for SetSize {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "set_size(width: {}, height: {})",
            self.width, self.height
        )
    }
}

pub(super) struct SetAnchorRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}
impl RequestParser<'_> for SetAnchorRect {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            x: parser.int()?,
            y: parser.int()?,
            width: parser.int()?,
            height: parser.int()?,
        })
    }
}
impl Debug for SetAnchorRect {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "set_anchor_rect(x: {}, y: {}, width: {}, height: {})",
            self.x, self.y, self.width, self.height
        )
    }
}

pub(super) struct SetAnchor {
    pub anchor: u32,
}
impl RequestParser<'_> for SetAnchor {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            anchor: parser.uint()?,
        })
    }
}
impl Debug for SetAnchor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "set_anchor(anchor: {})", self.anchor)
    }
}

pub(super) struct SetGravity {
    pub gravity: u32,
}
impl RequestParser<'_> for SetGravity {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            gravity: parser.uint()?,
        })
    }
}
impl Debug for SetGravity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "set_gravity(gravity: {})", self.gravity)
    }
}

pub(super) struct SetConstraintAdjustment {
    pub constraint_adjustment: u32,
}
impl RequestParser<'_> for SetConstraintAdjustment {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            constraint_adjustment: parser.uint()?,
        })
    }
}
impl Debug for SetConstraintAdjustment {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "set_constraint_adjustment(constraint_adjustment: {})",
            self.constraint_adjustment
        )
    }
}

pub(super) struct SetOffset {
    pub x: i32,
    pub y: i32,
}
impl RequestParser<'_> for SetOffset {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            x: parser.int()?,
            y: parser.int()?,
        })
    }
}
impl Debug for SetOffset {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "set_offset(x: {}, y: {})", self.x, self.y)
    }
}

pub(super) struct SetReactive;
impl RequestParser<'_> for SetReactive {
    fn parse(_parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self)
    }
}
impl Debug for SetReactive {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "set_reactive()")
    }
}

pub(super) struct SetParentSize {
    pub parent_width: i32,
    pub parent_height: i32,
}
impl RequestParser<'_> for SetParentSize {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            parent_width: parser.int()?,
            parent_height: parser.int()?,
        })
    }
}
impl Debug for SetParentSize {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "set_parent_size(parent_width: {}, parent_height: {})",
            self.parent_width, self.parent_height
        )
    }
}

pub(super) struct SetParentConfigure {
    pub serial: u32,
}
impl RequestParser<'_> for SetParentConfigure {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            serial: parser.uint()?,
        })
    }
}
impl Debug for SetParentConfigure {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "set_parent_configure(serial: {})", self.serial)
    }
}
