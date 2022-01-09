use crate::client::{ClientError, EventFormatter, RequestParser};
use crate::ifs::wl_seat::wl_keyboard::{
    WlKeyboard, ENTER, KEY, KEYMAP, LEAVE, MODIFIERS, REPEAT_INFO,
};
use crate::ifs::wl_surface::WlSurfaceId;
use crate::object::Object;
use crate::utils::buffd::{MsgFormatter, MsgParser, MsgParserError};
use std::fmt::{Debug, Formatter};
use std::ops::Deref;
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;

#[derive(Debug, Error)]
pub enum WlKeyboardError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Could not process a `release` request")]
    ReleaseError(#[from] ReleaseError),
    #[error("Could not create a keymap memfd")]
    KeymapMemfd(#[source] std::io::Error),
    #[error("Could not copy the keymap")]
    KeymapCopy(#[source] std::io::Error),
}
efrom!(WlKeyboardError, ClientError, ClientError);

#[derive(Debug, Error)]
pub enum ReleaseError {
    #[error("Parsing failed")]
    ParseError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ReleaseError, ParseError, MsgParserError);
efrom!(ReleaseError, ClientError, ClientError);

pub(super) struct Release;
impl RequestParser<'_> for Release {
    fn parse(_parser: &mut MsgParser<'_, '_>) -> Result<Self, MsgParserError> {
        Ok(Self)
    }
}
impl Debug for Release {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "destroy()",)
    }
}

pub(super) struct Keymap {
    pub obj: Rc<WlKeyboard>,
    pub format: u32,
    pub fd: Rc<OwnedFd>,
    pub size: u32,
}
impl EventFormatter for Keymap {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, KEYMAP)
            .uint(self.format)
            .fd(self.fd)
            .uint(self.size);
    }
    fn obj(&self) -> &dyn Object {
        self.obj.deref()
    }
}
impl Debug for Keymap {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "keymap(format: {}, fd: {}, size: {})",
            self.format,
            self.fd.raw(),
            self.size
        )
    }
}

pub(super) struct Enter {
    pub obj: Rc<WlKeyboard>,
    pub serial: u32,
    pub surface: WlSurfaceId,
    pub keys: Vec<u32>,
}
impl EventFormatter for Enter {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, ENTER)
            .uint(self.serial)
            .object(self.surface)
            .array(|f| {
                for &key in &self.keys {
                    f.uint(key);
                }
            });
    }
    fn obj(&self) -> &dyn Object {
        self.obj.deref()
    }
}
impl Debug for Enter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "enter(serial: {}, surface: {}, keys: {:?})",
            self.serial, self.surface, self.keys
        )
    }
}

pub(super) struct Leave {
    pub obj: Rc<WlKeyboard>,
    pub serial: u32,
    pub surface: WlSurfaceId,
}
impl EventFormatter for Leave {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, LEAVE)
            .uint(self.serial)
            .object(self.surface);
    }
    fn obj(&self) -> &dyn Object {
        self.obj.deref()
    }
}
impl Debug for Leave {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "leave(serial: {}, surface: {})",
            self.serial, self.surface
        )
    }
}

pub(super) struct Key {
    pub obj: Rc<WlKeyboard>,
    pub serial: u32,
    pub time: u32,
    pub key: u32,
    pub state: u32,
}
impl EventFormatter for Key {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, KEY)
            .uint(self.serial)
            .uint(self.time)
            .uint(self.key)
            .uint(self.state);
    }
    fn obj(&self) -> &dyn Object {
        self.obj.deref()
    }
}
impl Debug for Key {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "key(serial: {}, time: {}, key: {}, state: {})",
            self.serial, self.time, self.key, self.state
        )
    }
}

pub(super) struct Modifiers {
    pub obj: Rc<WlKeyboard>,
    pub serial: u32,
    pub mods_depressed: u32,
    pub mods_latched: u32,
    pub mods_locked: u32,
    pub group: u32,
}
impl EventFormatter for Modifiers {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, MODIFIERS)
            .uint(self.serial)
            .uint(self.mods_depressed)
            .uint(self.mods_latched)
            .uint(self.mods_locked)
            .uint(self.group);
    }
    fn obj(&self) -> &dyn Object {
        self.obj.deref()
    }
}
impl Debug for Modifiers {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "modifiers(serial: {}, mods_depressed: {}, mods_latched: {}, mods_locked: {}, group: {})", self.serial, self.mods_depressed, self.mods_latched, self.mods_locked, self.group)
    }
}

pub(super) struct RepeatInfo {
    pub obj: Rc<WlKeyboard>,
    pub rate: i32,
    pub delay: i32,
}
impl EventFormatter for RepeatInfo {
    fn format(self: Box<Self>, fmt: &mut MsgFormatter<'_>) {
        fmt.header(self.obj.id, REPEAT_INFO)
            .int(self.rate)
            .int(self.delay);
    }
    fn obj(&self) -> &dyn Object {
        self.obj.deref()
    }
}
impl Debug for RepeatInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "repeat_info(rate: {}, delay: {})", self.rate, self.delay)
    }
}
