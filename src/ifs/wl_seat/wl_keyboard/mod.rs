mod types;

use crate::client::{AddObj, DynEventFormatter};
use crate::ifs::wl_seat::WlSeatObj;
use crate::ifs::wl_surface::WlSurfaceId;
use crate::object::{Interface, Object, ObjectId};
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
pub use types::*;
use uapi::OwnedFd;

const RELEASE: u32 = 0;

const KEYMAP: u32 = 0;
const ENTER: u32 = 1;
const LEAVE: u32 = 2;
const KEY: u32 = 3;
const MODIFIERS: u32 = 4;
const REPEAT_INFO: u32 = 5;

#[allow(dead_code)]
const NO_KEYMAP: u32 = 0;
pub(super) const XKB_V1: u32 = 1;

#[allow(dead_code)]
const RELEASED: u32 = 0;
#[allow(dead_code)]
const PRESSED: u32 = 1;

id!(WlKeyboardId);

pub struct WlKeyboard {
    id: WlKeyboardId,
    seat: Rc<WlSeatObj>,
}

impl WlKeyboard {
    pub fn new(id: WlKeyboardId, seat: &Rc<WlSeatObj>) -> Self {
        Self {
            id,
            seat: seat.clone(),
        }
    }

    pub fn keymap(self: &Rc<Self>, format: u32, fd: Rc<OwnedFd>, size: u32) -> DynEventFormatter {
        Box::new(Keymap {
            obj: self.clone(),
            format,
            fd,
            size,
        })
    }

    #[allow(dead_code)]
    pub fn enter(
        self: &Rc<Self>,
        serial: u32,
        surface: WlSurfaceId,
        keys: Vec<u32>,
    ) -> DynEventFormatter {
        Box::new(Enter {
            obj: self.clone(),
            serial,
            surface,
            keys,
        })
    }

    #[allow(dead_code)]
    pub fn leave(self: &Rc<Self>, serial: u32, surface: WlSurfaceId) -> DynEventFormatter {
        Box::new(Leave {
            obj: self.clone(),
            serial,
            surface,
        })
    }

    #[allow(dead_code)]
    pub fn key(self: &Rc<Self>, serial: u32, time: u32, key: u32, state: u32) -> DynEventFormatter {
        Box::new(Key {
            obj: self.clone(),
            serial,
            time,
            key,
            state,
        })
    }

    #[allow(dead_code)]
    pub fn modifiers(
        self: &Rc<Self>,
        serial: u32,
        mods_depressed: u32,
        mods_latched: u32,
        mods_locked: u32,
        group: u32,
    ) -> DynEventFormatter {
        Box::new(Modifiers {
            obj: self.clone(),
            serial,
            mods_depressed,
            mods_latched,
            mods_locked,
            group,
        })
    }

    #[allow(dead_code)]
    pub fn repeat_info(self: &Rc<Self>, rate: i32, delay: i32) -> DynEventFormatter {
        Box::new(RepeatInfo {
            obj: self.clone(),
            rate,
            delay,
        })
    }

    async fn release(&self, parser: MsgParser<'_, '_>) -> Result<(), ReleaseError> {
        let _req: Release = self.seat.client.parse(self, parser)?;
        self.seat.keyboards.remove(&self.id);
        self.seat.client.remove_obj(self).await?;
        Ok(())
    }

    async fn handle_request_(
        &self,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), WlKeyboardError> {
        match request {
            RELEASE => self.release(parser).await?,
            _ => unreachable!(),
        }
        Ok(())
    }
}

handle_request!(WlKeyboard);

impl Object for WlKeyboard {
    fn id(&self) -> ObjectId {
        self.id.into()
    }

    fn interface(&self) -> Interface {
        Interface::WlKeyboard
    }

    fn num_requests(&self) -> u32 {
        RELEASE + 1
    }
}
