use super::CONFIGURE;
use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::ifs::wl_seat::WlSeatId;
use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::{XdgToplevel, XdgToplevelId, CLOSE};
use crate::object::Object;
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use bstr::BStr;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum XdgToplevelError {
    #[error("Could not process `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process `set_parent` request")]
    SetParentError(#[from] SetParentError),
    #[error("Could not process `set_title` request")]
    SetTitleError(#[from] SetTitleError),
    #[error("Could not process `set_app_id` request")]
    SetAppIdError(#[from] SetAppIdError),
    #[error("Could not process `show_window_menu` request")]
    ShowWindowMenuError(#[from] ShowWindowMenuError),
    #[error("Could not process `move` request")]
    MoveError(#[from] MoveError),
    #[error("Could not process `resize` request")]
    ResizeError(#[from] ResizeError),
    #[error("Could not process `set_max_size` request")]
    SetMaxSizeError(#[from] SetMaxSizeError),
    #[error("Could not process `set_min_size` request")]
    SetMinSizeError(#[from] SetMinSizeError),
    #[error("Could not process `set_maximized` request")]
    SetMaximizedError(#[from] SetMaximizedError),
    #[error("Could not process `unset_maximized` request")]
    UnsetMaximizedError(#[from] UnsetMaximizedError),
    #[error("Could not process `set_fullscreen` request")]
    SetFullscreenError(#[from] SetFullscreenError),
    #[error("Could not process `unset_fullscreen` request")]
    UnsetFullscreenError(#[from] UnsetFullscreenError),
    #[error("Could not process `set_minimized` request")]
    SetMinimizedError(#[from] SetMinimizedError),
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
pub enum SetParentError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(SetParentError, ParseFailed, MsgParserError);
efrom!(SetParentError, ClientError);

#[derive(Debug, Error)]
pub enum SetTitleError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(SetTitleError, ParseFailed, MsgParserError);
efrom!(SetTitleError, ClientError);

#[derive(Debug, Error)]
pub enum SetAppIdError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(SetAppIdError, ParseFailed, MsgParserError);
efrom!(SetAppIdError, ClientError);

#[derive(Debug, Error)]
pub enum ShowWindowMenuError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ShowWindowMenuError, ParseFailed, MsgParserError);
efrom!(ShowWindowMenuError, ClientError);

#[derive(Debug, Error)]
pub enum MoveError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(MoveError, ParseFailed, MsgParserError);
efrom!(MoveError, ClientError);

#[derive(Debug, Error)]
pub enum ResizeError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ResizeError, ParseFailed, MsgParserError);
efrom!(ResizeError, ClientError);

#[derive(Debug, Error)]
pub enum SetMaxSizeError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("width/height must be non-negative")]
    NonNegative,
}
efrom!(SetMaxSizeError, ParseFailed, MsgParserError);
efrom!(SetMaxSizeError, ClientError);

#[derive(Debug, Error)]
pub enum SetMinSizeError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("width/height must be non-negative")]
    NonNegative,
}
efrom!(SetMinSizeError, ParseFailed, MsgParserError);
efrom!(SetMinSizeError, ClientError);

#[derive(Debug, Error)]
pub enum SetMaximizedError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(SetMaximizedError, ParseFailed, MsgParserError);
efrom!(SetMaximizedError, ClientError);

#[derive(Debug, Error)]
pub enum UnsetMaximizedError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(UnsetMaximizedError, ParseFailed, MsgParserError);
efrom!(UnsetMaximizedError, ClientError);

#[derive(Debug, Error)]
pub enum SetFullscreenError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(SetFullscreenError, ParseFailed, MsgParserError);
efrom!(SetFullscreenError, ClientError);

#[derive(Debug, Error)]
pub enum UnsetFullscreenError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(UnsetFullscreenError, ParseFailed, MsgParserError);
efrom!(UnsetFullscreenError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum SetMinimizedError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(SetMinimizedError, ParseFailed, MsgParserError);
efrom!(SetMinimizedError, ClientError, ClientError);

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

pub(super) struct SetParent {
    pub parent: XdgToplevelId,
}
impl RequestParser<'_> for SetParent {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            parent: parser.object()?,
        })
    }
}
impl Debug for SetParent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "set_parent(parent: {})", self.parent)
    }
}

pub(super) struct SetTitle<'a> {
    pub title: &'a BStr,
}
impl<'a> RequestParser<'a> for SetTitle<'a> {
    fn parse(parser: &mut MsgParser<'_, 'a>) -> Result<Self, MsgParserError> {
        Ok(Self {
            title: parser.string()?,
        })
    }
}
impl<'a> Debug for SetTitle<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "set_title(title: {:?})", self.title)
    }
}

pub(super) struct SetAppId<'a> {
    pub app_id: &'a BStr,
}
impl<'a> RequestParser<'a> for SetAppId<'a> {
    fn parse(parser: &mut MsgParser<'_, 'a>) -> Result<Self, MsgParserError> {
        Ok(Self {
            app_id: parser.string()?,
        })
    }
}
impl<'a> Debug for SetAppId<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "set_app_id(app_id: {:?})", self.app_id)
    }
}

pub(super) struct ShowWindowMenu {
    pub seat: WlSeatId,
    pub serial: u32,
    pub x: i32,
    pub y: i32,
}
impl RequestParser<'_> for ShowWindowMenu {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            seat: parser.object()?,
            serial: parser.uint()?,
            x: parser.int()?,
            y: parser.int()?,
        })
    }
}
impl Debug for ShowWindowMenu {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "show_window_menu(seat: {}, serial: {}, x: {}, y: {})",
            self.seat, self.serial, self.x, self.y
        )
    }
}

pub(super) struct Move {
    pub seat: WlSeatId,
    pub serial: u32,
}
impl RequestParser<'_> for Move {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            seat: parser.object()?,
            serial: parser.uint()?,
        })
    }
}
impl Debug for Move {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "move(seat: {}, serial: {})", self.seat, self.serial)
    }
}

pub(super) struct Resize {
    pub seat: WlSeatId,
    pub serial: u32,
    pub edges: u32,
}
impl RequestParser<'_> for Resize {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            seat: parser.object()?,
            serial: parser.uint()?,
            edges: parser.uint()?,
        })
    }
}
impl Debug for Resize {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "resize(seat: {}, serial: {}, edges: {})",
            self.seat, self.serial, self.edges
        )
    }
}

pub(super) struct SetMaxSize {
    pub width: i32,
    pub height: i32,
}
impl RequestParser<'_> for SetMaxSize {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            width: parser.int()?,
            height: parser.int()?,
        })
    }
}
impl Debug for SetMaxSize {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "set_max_size(width: {}, height: {})",
            self.width, self.height
        )
    }
}

pub(super) struct SetMinSize {
    pub width: i32,
    pub height: i32,
}
impl RequestParser<'_> for SetMinSize {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            width: parser.int()?,
            height: parser.int()?,
        })
    }
}
impl Debug for SetMinSize {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "set_min_size(width: {}, height: {})",
            self.width, self.height
        )
    }
}

pub(super) struct SetMaximized;
impl RequestParser<'_> for SetMaximized {
    fn parse(_parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self)
    }
}
impl Debug for SetMaximized {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "set_maximized()")
    }
}

pub(super) struct UnsetMaximized;
impl RequestParser<'_> for UnsetMaximized {
    fn parse(_parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self)
    }
}
impl Debug for UnsetMaximized {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "unset_maximized()")
    }
}

pub(super) struct SetFullscreen {
    pub output: crate::ifs::wl_output::WlOutputId,
}
impl RequestParser<'_> for SetFullscreen {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            output: parser.object()?,
        })
    }
}
impl Debug for SetFullscreen {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "set_fullscreen(output: {})", self.output)
    }
}

pub(super) struct UnsetFullscreen;
impl RequestParser<'_> for UnsetFullscreen {
    fn parse(_parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self)
    }
}
impl Debug for UnsetFullscreen {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "unset_fullscreen()")
    }
}

pub(super) struct SetMinimized;
impl RequestParser<'_> for SetMinimized {
    fn parse(_parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self)
    }
}
impl Debug for SetMinimized {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "set_minimized()")
    }
}

pub(super) struct Configure {
    pub obj: Rc<XdgToplevel>,
    pub width: i32,
    pub height: i32,
    pub states: Vec<u32>,
}
impl EventFormatter for Configure {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, CONFIGURE)
            .int(self.width)
            .int(self.height)
            .array(|fmt| {
                for &state in &self.states {
                    fmt.uint(state);
                }
            });
    }

    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Configure {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "configure(width: {}, height: {}, states: {:?})",
            self.width, self.height, self.states
        )
    }
}

pub(super) struct Close {
    pub obj: Rc<XdgToplevel>,
}
impl EventFormatter for Close {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, CLOSE);
    }

    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Close {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "close()")
    }
}
