
use crate::client::{Client, ClientError, DynEventFormatter};
use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::{Decoration, XdgToplevel};
use crate::object::Object;
use crate::utils::buffd::{MsgParser, MsgParserError};
use std::rc::Rc;
use thiserror::Error;
use crate::wire::zxdg_toplevel_decoration_v1::*;
use crate::wire::ZxdgToplevelDecorationV1Id;

const CLIENT_SIDE: u32 = 1;
const SERVER_SIDE: u32 = 2;

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
            self_id: self.id,
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

#[derive(Debug, Error)]
pub enum ZxdgToplevelDecorationV1Error {
    #[error("Could not process a `destroy` request")]
    DestoryError(#[from] DestroyError),
    #[error("Could not process a `set_mode` request")]
    SetModeError(#[from] SetModeError),
    #[error("Could not process a `unset_mode` request")]
    UnsetModeError(#[from] UnsetModeError),
}

#[derive(Debug, Error)]
pub enum DestroyError {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(DestroyError, ClientError);
efrom!(DestroyError, MsgParserError);

#[derive(Debug, Error)]
pub enum SetModeError {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
}
efrom!(SetModeError, MsgParserError);

#[derive(Debug, Error)]
pub enum UnsetModeError {
    #[error("Parsing failed")]
    MsgParserError(#[source] Box<MsgParserError>),
}
efrom!(UnsetModeError, MsgParserError);
