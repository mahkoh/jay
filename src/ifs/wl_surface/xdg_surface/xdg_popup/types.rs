use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::ifs::wl_surface::xdg_surface::xdg_popup::{
    XdgPopup, CONFIGURE, POPUP_DONE, REPOSITIONED,
};
use crate::object::{Object, ObjectId};
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum XdgPopupError {
    #[error("Could not process `destroy` request")]
    DestroyError(#[from] DestroyError),
    #[error("Could not process `grab` request")]
    GrabError(#[from] GrabError),
    #[error("Could not process `reposition` request")]
    RepositionError(#[from] RepositionError),
}

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ParseFailed, MsgParserError);
efrom!(DestroyError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum GrabError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(GrabError, ParseFailed, MsgParserError);
efrom!(GrabError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum RepositionError {
    #[error("Parsing failed")]
    ParseFailed(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(RepositionError, ParseFailed, MsgParserError);
efrom!(RepositionError, ClientError, ClientError);

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

pub(super) struct Grab {
    pub seat: ObjectId,
    pub serial: u32,
}
impl RequestParser<'_> for Grab {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            seat: parser.object()?,
            serial: parser.uint()?,
        })
    }
}
impl Debug for Grab {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "grab(seat: {}, serial: {})", self.seat, self.serial)
    }
}

pub(super) struct Reposition {
    pub positioner: ObjectId,
    pub token: u32,
}
impl RequestParser<'_> for Reposition {
    fn parse(parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self {
            positioner: parser.object()?,
            token: parser.uint()?,
        })
    }
}
impl Debug for Reposition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "reposition(positioner: {}, token: {})",
            self.positioner, self.token,
        )
    }
}

pub(super) struct Configure {
    pub obj: Rc<XdgPopup>,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}
impl EventFormatter for Configure {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, CONFIGURE)
            .int(self.x)
            .int(self.y)
            .int(self.width)
            .int(self.height);
    }

    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Configure {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "configure(x: {}, y: {}, width: {}, height: {})",
            self.x, self.y, self.width, self.height
        )
    }
}

pub(super) struct PopupDone {
    pub obj: Rc<XdgPopup>,
}
impl EventFormatter for PopupDone {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, POPUP_DONE);
    }

    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for PopupDone {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "popup_done()")
    }
}

pub(super) struct Repositioned {
    pub obj: Rc<XdgPopup>,
    pub token: u32,
}
impl EventFormatter for Repositioned {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, REPOSITIONED).uint(self.token);
    }

    fn obj(&self) -> &dyn Object {
        &*self.obj
    }
}
impl Debug for Repositioned {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "repositioned(token: {})", self.token)
    }
}
