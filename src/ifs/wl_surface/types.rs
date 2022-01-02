use crate::client::{ClientError, RequestParser};
use crate::object::ObjectId;
use crate::utils::buffd::{MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WlSurfaceError {
    #[error("Could not process `destroy` request")]
    DestroyError(#[source] Box<DestroyError>),
    #[error("Could not process `attach` request")]
    AttachError(#[source] Box<AttachError>),
    #[error("Could not process `damage` request")]
    DamageError(#[source] Box<DamageError>),
    #[error("Could not process `frame` request")]
    FrameError(#[source] Box<FrameError>),
    #[error("Could not process `set_opaque_region` request")]
    SetOpaqueRegionError(#[source] Box<SetOpaqueRegionError>),
    #[error("Could not process `set_input_region` request")]
    SetInputRegionError(#[source] Box<SetInputRegionError>),
    #[error("Could not process `commit` request")]
    CommitError(#[source] Box<CommitError>),
    #[error("Could not process `set_buffer_transform` request")]
    SetBufferTransformError(#[source] Box<SetBufferTransformError>),
    #[error("Could not process `set_buffer_scale_error` request")]
    SetBufferScaleError(#[source] Box<SetBufferScaleError>),
    #[error("Could not process `damage_buffer` request")]
    DamageBufferError(#[source] Box<DamageBufferError>),
}
efrom!(WlSurfaceError, DestroyError, DestroyError);
efrom!(WlSurfaceError, AttachError, AttachError);
efrom!(WlSurfaceError, DamageError, DamageError);
efrom!(WlSurfaceError, FrameError, FrameError);
efrom!(WlSurfaceError, SetOpaqueRegionError, SetOpaqueRegionError);
efrom!(WlSurfaceError, SetInputRegionError, SetInputRegionError);
efrom!(WlSurfaceError, CommitError, CommitError);
efrom!(
    WlSurfaceError,
    SetBufferTransformError,
    SetBufferTransformError
);
efrom!(WlSurfaceError, SetBufferScaleError, SetBufferScaleError);
efrom!(WlSurfaceError, DamageBufferError, DamageBufferError);

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
}
efrom!(DestroyError, ParseFailed, MsgParserError);

#[derive(Debug, Error)]
pub enum AttachError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
}
efrom!(AttachError, ParseFailed, MsgParserError);

#[derive(Debug, Error)]
pub enum DamageError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
}
efrom!(DamageError, ParseFailed, MsgParserError);

#[derive(Debug, Error)]
pub enum FrameError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
}
efrom!(FrameError, ParseFailed, MsgParserError);

#[derive(Debug, Error)]
pub enum SetOpaqueRegionError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(SetOpaqueRegionError, ParseFailed, MsgParserError);
efrom!(SetOpaqueRegionError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum SetInputRegionError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(SetInputRegionError, ParseFailed, MsgParserError);
efrom!(SetInputRegionError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum CommitError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
}
efrom!(CommitError, ParseFailed, MsgParserError);

#[derive(Debug, Error)]
pub enum SetBufferTransformError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
}
efrom!(SetBufferTransformError, ParseFailed, MsgParserError);

#[derive(Debug, Error)]
pub enum SetBufferScaleError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
}
efrom!(SetBufferScaleError, ParseFailed, MsgParserError);

#[derive(Debug, Error)]
pub enum DamageBufferError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
}
efrom!(DamageBufferError, ParseFailed, MsgParserError);

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

pub(super) struct Attach {
    pub buffer: ObjectId,
    pub x: i32,
    pub y: i32,
}
impl RequestParser<'_> for Attach {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            buffer: parser.object()?,
            x: parser.int()?,
            y: parser.int()?,
        })
    }
}
impl Debug for Attach {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "attach(buffer: {}, x: {}, y: {})",
            self.buffer, self.x, self.y
        )
    }
}

pub(super) struct Damage {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}
impl RequestParser<'_> for Damage {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            x: parser.int()?,
            y: parser.int()?,
            width: parser.int()?,
            height: parser.int()?,
        })
    }
}
impl Debug for Damage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "damage(x: {}, y: {}, width: {}, height: {})",
            self.x, self.y, self.width, self.height
        )
    }
}

pub(super) struct Frame {
    pub callback: ObjectId,
}
impl RequestParser<'_> for Frame {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            callback: parser.object()?,
        })
    }
}
impl Debug for Frame {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "frame(callback: {})", self.callback)
    }
}

pub(super) struct SetOpaqueRegion {
    pub region: ObjectId,
}
impl RequestParser<'_> for SetOpaqueRegion {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            region: parser.object()?,
        })
    }
}
impl Debug for SetOpaqueRegion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "set_opaque_region(region: {})", self.region)
    }
}

pub(super) struct SetInputRegion {
    pub region: ObjectId,
}
impl RequestParser<'_> for SetInputRegion {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            region: parser.object()?,
        })
    }
}
impl Debug for SetInputRegion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "set_input_region(region: {})", self.region)
    }
}

pub(super) struct Commit;
impl RequestParser<'_> for Commit {
    fn parse(_parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self)
    }
}
impl Debug for Commit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "commit()")
    }
}

pub(super) struct SetBufferTransform {
    pub transform: i32,
}
impl RequestParser<'_> for SetBufferTransform {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            transform: parser.int()?,
        })
    }
}
impl Debug for SetBufferTransform {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "set_buffer_transform(transform: {})", self.transform)
    }
}

pub(super) struct SetBufferScale {
    pub scale: i32,
}
impl RequestParser<'_> for SetBufferScale {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            scale: parser.int()?,
        })
    }
}
impl Debug for SetBufferScale {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "set_buffer_scale(scale: {})", self.scale)
    }
}

pub(super) struct DamageBuffer {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}
impl RequestParser<'_> for DamageBuffer {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            x: parser.int()?,
            y: parser.int()?,
            width: parser.int()?,
            height: parser.int()?,
        })
    }
}
impl Debug for DamageBuffer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "damage_buffer(x: {}, y: {}, width: {}, height: {})",
            self.x, self.y, self.width, self.height
        )
    }
}
