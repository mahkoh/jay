use {
    crate::{
        client::{Client, ClientError},
        ifs::wl_surface::xdg_surface::xdg_toplevel::{Decoration, XdgToplevel},
        leaks::Tracker,
        object::{Object, Version},
        wire::{zxdg_toplevel_decoration_v1::*, ZxdgToplevelDecorationV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

const CLIENT_SIDE: u32 = 1;
const SERVER_SIDE: u32 = 2;

pub struct ZxdgToplevelDecorationV1 {
    pub id: ZxdgToplevelDecorationV1Id,
    pub client: Rc<Client>,
    pub toplevel: Rc<XdgToplevel>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl ZxdgToplevelDecorationV1 {
    pub fn new(
        id: ZxdgToplevelDecorationV1Id,
        client: &Rc<Client>,
        toplevel: &Rc<XdgToplevel>,
        version: Version,
    ) -> Self {
        Self {
            id,
            client: client.clone(),
            toplevel: toplevel.clone(),
            tracker: Default::default(),
            version,
        }
    }

    fn send_configure(&self, mode: u32) {
        self.client.event(Configure {
            self_id: self.id,
            mode,
        })
    }

    pub fn do_send_configure(&self) {
        let mode = match self.toplevel.decoration.get() {
            Decoration::Client => CLIENT_SIDE,
            Decoration::Server => SERVER_SIDE,
        };
        self.send_configure(mode);
        self.toplevel.send_current_configure();
    }
}

impl ZxdgToplevelDecorationV1RequestHandler for ZxdgToplevelDecorationV1 {
    type Error = ZxdgToplevelDecorationV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn set_mode(&self, _req: SetMode, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.do_send_configure();
        Ok(())
    }

    fn unset_mode(&self, _req: UnsetMode, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.do_send_configure();
        Ok(())
    }
}

object_base! {
    self = ZxdgToplevelDecorationV1;
    version = self.version;
}

impl Object for ZxdgToplevelDecorationV1 {}

simple_add_obj!(ZxdgToplevelDecorationV1);

#[derive(Debug, Error)]
pub enum ZxdgToplevelDecorationV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(ZxdgToplevelDecorationV1Error, ClientError);
