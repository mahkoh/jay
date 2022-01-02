use crate::client::ClientError;
use crate::ifs::wl_display::WlDisplay;
use crate::utils::buffd::MsgParser;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

pub const WL_DISPLAY_ID: ObjectId = ObjectId(1);

#[derive(Debug, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct ObjectId(u32);

impl ObjectId {
    pub fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub fn raw(self) -> u32 {
        self.0
    }
}

impl Display for ObjectId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

pub trait ObjectHandleRequest {
    fn handle_request<'a>(
        self: Rc<Self>,
        request: u32,
        parser: MsgParser<'a, 'a>,
    ) -> Pin<Box<dyn Future<Output = Result<(), ClientError>> + 'a>>;
}

pub trait Object: ObjectHandleRequest + 'static {
    fn id(&self) -> ObjectId;
    fn interface(&self) -> Interface;
    fn num_requests(&self) -> u32;
    fn into_display(self: Rc<Self>) -> Result<Rc<WlDisplay>, ClientError> {
        Err(ClientError::NotADisplay(self.id()))
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Interface {
    WlDisplay,
    WlCallback,
    WlCompositor,
    WlRegistry,
    WlShm,
    WlShmPool,
    WlSubcompositor,
    XdgWmBase,
    WlSurface,
    WlSubsurface,
    WlRegion,
}

impl Interface {
    pub fn name(self) -> &'static str {
        match self {
            Interface::WlDisplay => "wl_display",
            Interface::WlCallback => "wl_callback",
            Interface::WlCompositor => "wl_compositor",
            Interface::WlRegistry => "wl_registry",
            Interface::WlShm => "wl_shm",
            Interface::WlSubcompositor => "wl_subcompositor",
            Interface::XdgWmBase => "xdg_wm_base",
            Interface::WlSurface => "wl_surface",
            Interface::WlSubsurface => "wl_subsurface",
            Interface::WlShmPool => "wl_shm_pool",
            Interface::WlRegion => "wl_region",
        }
    }
}
