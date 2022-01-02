use crate::objects::ObjectId;
use crate::utils::buffd::{WlParser, WlParserError};
use crate::wl_client::{RequestParser, WlClientError};
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
    ParseFailed(#[source] Box<WlParserError>),
}
efrom!(DestroyError, ParseFailed, WlParserError);

#[derive(Debug, Error)]
pub enum AttachError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<WlParserError>),
}
efrom!(AttachError, ParseFailed, WlParserError);

#[derive(Debug, Error)]
pub enum DamageError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<WlParserError>),
}
efrom!(DamageError, ParseFailed, WlParserError);

#[derive(Debug, Error)]
pub enum FrameError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<WlParserError>),
}
efrom!(FrameError, ParseFailed, WlParserError);

#[derive(Debug, Error)]
pub enum SetOpaqueRegionError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<WlParserError>),
    #[error(transparent)]
    ClientError(Box<WlClientError>),
}
efrom!(SetOpaqueRegionError, ParseFailed, WlParserError);
efrom!(SetOpaqueRegionError, ClientError, WlClientError);

#[derive(Debug, Error)]
pub enum SetInputRegionError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<WlParserError>),
    #[error(transparent)]
    ClientError(Box<WlClientError>),
}
efrom!(SetInputRegionError, ParseFailed, WlParserError);
efrom!(SetInputRegionError, ClientError, WlClientError);

#[derive(Debug, Error)]
pub enum CommitError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<WlParserError>),
}
efrom!(CommitError, ParseFailed, WlParserError);

#[derive(Debug, Error)]
pub enum SetBufferTransformError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<WlParserError>),
}
efrom!(SetBufferTransformError, ParseFailed, WlParserError);

#[derive(Debug, Error)]
pub enum SetBufferScaleError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<WlParserError>),
}
efrom!(SetBufferScaleError, ParseFailed, WlParserError);

#[derive(Debug, Error)]
pub enum DamageBufferError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<WlParserError>),
}
efrom!(DamageBufferError, ParseFailed, WlParserError);

pub(super) struct Destroy;
impl RequestParser<'_> for Destroy {
    fn parse(_parser: &mut WlParser<'_, '_>) -> Result<Self, WlParserError> {
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
    fn parse(parser: &mut WlParser<'_, '_>) -> Result<Self, WlParserError> {
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
    fn parse(parser: &mut WlParser<'_, '_>) -> Result<Self, WlParserError> {
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
    fn parse(parser: &mut WlParser<'_, '_>) -> Result<Self, WlParserError> {
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
    fn parse(parser: &mut WlParser<'_, '_>) -> Result<Self, WlParserError> {
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
    fn parse(parser: &mut WlParser<'_, '_>) -> Result<Self, WlParserError> {
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
    fn parse(_parser: &mut WlParser<'_, '_>) -> Result<Self, WlParserError> {
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
    fn parse(parser: &mut WlParser<'_, '_>) -> Result<Self, WlParserError> {
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
    fn parse(parser: &mut WlParser<'_, '_>) -> Result<Self, WlParserError> {
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
    fn parse(parser: &mut WlParser<'_, '_>) -> Result<Self, WlParserError> {
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
