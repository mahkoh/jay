use crate::client::ClientError;
use crate::ifs::wl_seat::WlSeat;
use crate::object::Object;
use crate::utils::buffd::MsgParser;
use crate::utils::buffd::MsgParserError;
use crate::wire::wl_keyboard::*;
use crate::wire::{WlKeyboardId, WlSurfaceId};
use std::rc::Rc;
use thiserror::Error;
use uapi::{c, Errno, OwnedFd};

pub const REPEAT_INFO_SINCE: u32 = 4;

#[allow(dead_code)]
const NO_KEYMAP: u32 = 0;
pub(super) const XKB_V1: u32 = 1;

pub(super) const RELEASED: u32 = 0;
pub(super) const PRESSED: u32 = 1;

pub struct WlKeyboard {
    id: WlKeyboardId,
    seat: Rc<WlSeat>,
}

impl WlKeyboard {
    pub fn new(id: WlKeyboardId, seat: &Rc<WlSeat>) -> Self {
        Self {
            id,
            seat: seat.clone(),
        }
    }

    pub fn needs_dedicated_keymap_fd(&self) -> bool {
        self.seat.version < 7
    }

    pub fn keymap_fd(&self) -> Result<Rc<OwnedFd>, WlKeyboardError> {
        if !self.needs_dedicated_keymap_fd() {
            return Ok(self.seat.global.layout.clone());
        }
        let fd = match uapi::memfd_create("shared-keymap", c::MFD_CLOEXEC) {
            Ok(fd) => fd,
            Err(e) => return Err(WlKeyboardError::KeymapMemfd(e.into())),
        };
        let target = self.seat.global.layout_size as c::off_t;
        let mut pos = 0;
        while pos < target {
            let rem = target - pos;
            let res = uapi::sendfile(
                fd.raw(),
                self.seat.global.layout.raw(),
                Some(&mut pos),
                rem as usize,
            );
            match res {
                Ok(_) | Err(Errno(c::EINTR)) => {}
                Err(e) => return Err(WlKeyboardError::KeymapCopy(e.into())),
            }
        }
        Ok(Rc::new(fd))
    }

    pub fn send_keymap(self: &Rc<Self>, format: u32, fd: Rc<OwnedFd>, size: u32) {
        self.seat.client.event(Keymap {
            self_id: self.id,
            format,
            fd,
            size,
        })
    }

    pub fn send_enter(self: &Rc<Self>, serial: u32, surface: WlSurfaceId, keys: &[u32]) {
        self.seat.client.event(Enter {
            self_id: self.id,
            serial,
            surface,
            keys,
        })
    }

    pub fn send_leave(self: &Rc<Self>, serial: u32, surface: WlSurfaceId) {
        self.seat.client.event(Leave {
            self_id: self.id,
            serial,
            surface,
        })
    }

    pub fn send_key(self: &Rc<Self>, serial: u32, time: u32, key: u32, state: u32) {
        self.seat.client.event(Key {
            self_id: self.id,
            serial,
            time,
            key,
            state,
        })
    }

    pub fn send_modifiers(
        self: &Rc<Self>,
        serial: u32,
        mods_depressed: u32,
        mods_latched: u32,
        mods_locked: u32,
        group: u32,
    ) {
        self.seat.client.event(Modifiers {
            self_id: self.id,
            serial,
            mods_depressed,
            mods_latched,
            mods_locked,
            group,
        })
    }

    pub fn send_repeat_info(self: &Rc<Self>, rate: i32, delay: i32) {
        self.seat.client.event(RepeatInfo {
            self_id: self.id,
            rate,
            delay,
        })
    }

    fn release(&self, parser: MsgParser<'_, '_>) -> Result<(), ReleaseError> {
        let _req: Release = self.seat.client.parse(self, parser)?;
        self.seat.keyboards.remove(&self.id);
        self.seat.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    WlKeyboard, WlKeyboardError;

    RELEASE => release,
}

impl Object for WlKeyboard {
    fn num_requests(&self) -> u32 {
        RELEASE + 1
    }
}

simple_add_obj!(WlKeyboard);

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
