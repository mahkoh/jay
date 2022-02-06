use crate::client::ClientError;
use crate::utils::buffd::MsgParser;
use std::fmt::{Display, Formatter};
use std::rc::Rc;
use crate::wire::WlDisplayId;

pub const WL_DISPLAY_ID: WlDisplayId = WlDisplayId::from_raw(1);

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

pub trait ObjectBase {
    fn id(&self) -> ObjectId;
    fn handle_request(
        self: Rc<Self>,
        request: u32,
        parser: MsgParser<'_, '_>,
    ) -> Result<(), ClientError>;
    fn interface(&self) -> Interface;
}

pub trait Object: ObjectBase + 'static {
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
    WlDataDeviceManager,
    WlDataDevice,
    WlDataSource,
    WlDataOffer,
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
    WlDrm,
    ZwpLinuxDmabufV1,
    ZwpLinuxDmabufFeedbackV1,
    ZwpLinuxBufferParamsV1,
    ZxdgDecorationManagerV1,
    ZxdgToplevelDecorationV1,
    OrgKdeKwinServerDecorationManager,
    OrgKdeKwinServerDecoration,
    ZwpPrimarySelectionDeviceManagerV1,
    ZwpPrimarySelectionDeviceV1,
    ZwpPrimarySelectionSourceV1,
    ZwpPrimarySelectionOfferV1,
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
            Interface::WlDataDeviceManager => "wl_data_device_manager",
            Interface::WlDataDevice => "wl_data_device",
            Interface::WlDataSource => "wl_data_source",
            Interface::WlDataOffer => "wl_data_offer",
            Interface::ZwpLinuxDmabufV1 => "zwp_linux_dmabuf_v1",
            Interface::ZwpLinuxDmabufFeedbackV1 => "zwp_linux_dmabuf_feedback_v1",
            Interface::ZwpLinuxBufferParamsV1 => "zwp_linux_buffer_params_v1",
            Interface::WlDrm => "wl_drm",
            Interface::ZxdgDecorationManagerV1 => "zxdg_decoration_manager_v1",
            Interface::ZxdgToplevelDecorationV1 => "zxdg_toplevel_decoration_v1",
            Interface::OrgKdeKwinServerDecorationManager => {
                "org_kde_kwin_server_decoration_manager"
            }
            Interface::OrgKdeKwinServerDecoration => "org_kde_kwin_server_decoration",
            Interface::ZwpPrimarySelectionDeviceManagerV1 => {
                "zwp_primary_selection_device_manager_v1"
            }
            Interface::ZwpPrimarySelectionDeviceV1 => "zwp_primary_selection_device_v1",
            Interface::ZwpPrimarySelectionSourceV1 => "zwp_primary_selection_source_v1",
            Interface::ZwpPrimarySelectionOfferV1 => "zwp_primary_selection_offer_v1",
        }
    }
}
