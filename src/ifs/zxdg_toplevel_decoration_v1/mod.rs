mod types;

use crate::client::{Client, DynEventFormatter};
use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::{Decoration, XdgToplevel};
use crate::object::Object;
use crate::utils::buffd::MsgParser;
use std::rc::Rc;
pub use types::*;

const DESTROY: u32 = 0;
const SET_MODE: u32 = 1;
const UNSET_MODE: u32 = 2;

const CONFIGURE: u32 = 0;

const CLIENT_SIDE: u32 = 1;
const SERVER_SIDE: u32 = 2;

id!(ZxdgToplevelDecorationV1Id);

pub struct ZxdgToplevelDecorationV1 {
    pub id: ZxdgToplevelDecorationV1Id,
    pub client: Rc<Client>,
    pub toplevel: Rc<XdgToplevel>,
}

impl ZxdgToplevelDecorationV1 {
    pub fn new(
        id: ZxdgToplevelDecorationV1Id,
        client: &Rc<Client>,
        toplevel: &Rc<XdgToplevel>,
    ) -> Self {
        Self {
            id,
            client: client.clone(),
            toplevel: toplevel.clone(),
        }
    }

    fn configure(self: &Rc<Self>, mode: u32) -> DynEventFormatter {
        Box::new(Configure {
            obj: self.clone(),
            mode,
        })
    }

    pub fn send_configure(self: &Rc<Self>) {
        let mode = match self.toplevel.decoration.get() {
            Decoration::Client => CLIENT_SIDE,
            Decoration::Server => SERVER_SIDE,
        };
        self.client.event(self.configure(mode));
        self.toplevel.xdg.send_configure();
    }

    fn destroy(&self, parser: MsgParser<'_, '_>) -> Result<(), DestroyError> {
        let _req: Destroy = self.client.parse(self, parser)?;
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_mode(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), SetModeError> {
        let _req: SetMode = self.client.parse(&**self, parser)?;
        self.send_configure();
        Ok(())
    }

    fn unset_mode(self: &Rc<Self>, parser: MsgParser<'_, '_>) -> Result<(), UnsetModeError> {
        let _req: UnsetMode = self.client.parse(&**self, parser)?;
        self.send_configure();
        Ok(())
    }
}

object_base! {
    ZxdgToplevelDecorationV1, ZxdgToplevelDecorationV1Error;

    DESTROY => destroy,
    SET_MODE => set_mode,
    UNSET_MODE => unset_mode,
}

impl Object for ZxdgToplevelDecorationV1 {
    fn num_requests(&self) -> u32 {
        UNSET_MODE + 1
    }
}

simple_add_obj!(ZxdgToplevelDecorationV1);
