use crate::client::ClientError;
use crate::utils::buffd::MsgParser;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

pub const WL_DISPLAY_ID: ObjectId = ObjectId(1);

#[derive(Debug, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct ObjectId(u32);

impl ObjectId {
    pub const NONE: Self = ObjectId(0);

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
    fn break_loops(&self) {}
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Interface {
    WlDisplay,
    WlCallback,
    WlCompositor,
    WlOutput,
    WlRegistry,
    WlShm,
    WlShmPool,
    WlTouch,
    WlPointer,
    WlKeyboard,
    WlSubcompositor,
    XdgWmBase,
    XdgPositioner,
    WlSurface,
    WlSubsurface,
    XdgSurface,
    XdgPopup,
    XdgToplevel,
    WlRegion,
    WlBuffer,
    WlSeat,
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
            Interface::XdgSurface => "xdg_surface",
            Interface::XdgPositioner => "xdg_positioner",
            Interface::XdgPopup => "xdg_popup",
            Interface::XdgToplevel => "xdg_toplevel",
            Interface::WlBuffer => "wl_buffer",
            Interface::WlOutput => "wl_output",
            Interface::WlSeat => "wl_seat",
            Interface::WlTouch => "wl_touch",
            Interface::WlPointer => "wl_pointer",
            Interface::WlKeyboard => "wl_keyboard",
        }
    }
}
