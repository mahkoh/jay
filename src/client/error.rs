use crate::client::ClientId;
use crate::ifs::wl_buffer::{WlBufferError, WlBufferId};
use crate::ifs::wl_compositor::WlCompositorError;
use crate::ifs::wl_data_device::WlDataDeviceError;
use crate::ifs::wl_data_device_manager::WlDataDeviceManagerError;
use crate::ifs::wl_data_offer::WlDataOfferError;
use crate::ifs::wl_data_source::WlDataSourceError;
use crate::ifs::wl_display::WlDisplayError;
use crate::ifs::wl_drm::WlDrmError;
use crate::ifs::wl_output::WlOutputError;
use crate::ifs::wl_region::{WlRegionError, WlRegionId};
use crate::ifs::wl_registry::WlRegistryError;
use crate::ifs::wl_seat::wl_keyboard::WlKeyboardError;
use crate::ifs::wl_seat::wl_pointer::WlPointerError;
use crate::ifs::wl_seat::wl_touch::WlTouchError;
use crate::ifs::wl_seat::{WlSeatError, WlSeatId};
use crate::ifs::wl_shm::WlShmError;
use crate::ifs::wl_shm_pool::WlShmPoolError;
use crate::ifs::wl_subcompositor::WlSubcompositorError;
use crate::ifs::wl_surface::wl_subsurface::WlSubsurfaceError;
use crate::ifs::wl_surface::xdg_surface::xdg_popup::XdgPopupError;
use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::{XdgToplevelError, XdgToplevelId};
use crate::ifs::wl_surface::xdg_surface::{XdgSurfaceError, XdgSurfaceId};
use crate::ifs::wl_surface::{WlSurfaceError, WlSurfaceId};
use crate::ifs::xdg_positioner::{XdgPositionerError, XdgPositionerId};
use crate::ifs::xdg_wm_base::XdgWmBaseError;
use crate::ifs::zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1Error;
use crate::ifs::zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1Error;
use crate::ifs::zxdg_decoration_manager_v1::ZxdgDecorationManagerV1Error;
use crate::ifs::zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1Error;
use crate::object::ObjectId;
use crate::utils::buffd::{BufFdError, MsgParserError};
use crate::AsyncError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("An error occurred in the async engine")]
    Async(#[from] AsyncError),
    #[error("An error occurred reading from/writing to the client")]
    Io(#[from] BufFdError),
    #[error("An error occurred while processing a request")]
    RequestError(#[source] Box<ClientError>),
    #[error("Client tried to invoke a non-existent method")]
    InvalidMethod,
    #[error("Client tried to access non-existent object {0}")]
    InvalidObject(ObjectId),
    #[error("The message size is < 8")]
    MessageSizeTooSmall,
    #[error("The size of the message is not a multiple of 4")]
    UnalignedMessage,
    #[error("The requested client {0} does not exist")]
    ClientDoesNotExist(ClientId),
    #[error("There is no wl_region with id {0}")]
    RegionDoesNotExist(WlRegionId),
    #[error("There is no wl_buffer with id {0}")]
    BufferDoesNotExist(WlBufferId),
    #[error("There is no wl_surface with id {0}")]
    SurfaceDoesNotExist(WlSurfaceId),
    #[error("There is no xdg_surface with id {0}")]
    XdgSurfaceDoesNotExist(XdgSurfaceId),
    #[error("There is no xdg_toplevel with id {0}")]
    XdgToplevelDoesNotExist(XdgToplevelId),
    #[error("There is no xdg_positioner with id {0}")]
    XdgPositionerDoesNotExist(XdgPositionerId),
    #[error("There is no wl_seat with id {0}")]
    WlSeatDoesNotExist(WlSeatId),
    #[error("Cannot parse the message")]
    ParserError(#[source] Box<MsgParserError>),
    #[error("Server tried to allocate more than 0x1_00_00_00 ids")]
    TooManyIds,
    #[error("The server object id is out of bounds")]
    ServerIdOutOfBounds,
    #[error("The object id is unknown")]
    UnknownId,
    #[error("The id is already in use")]
    IdAlreadyInUse,
    #[error("The client object id is out of bounds")]
    ClientIdOutOfBounds,
    #[error("An error occurred in a `wl_display`")]
    WlDisplayError(#[source] Box<WlDisplayError>),
    #[error("An error occurred in a `wl_registry`")]
    WlRegistryError(#[source] Box<WlRegistryError>),
    #[error("Could not add object {0} to the client")]
    AddObjectError(ObjectId, #[source] Box<ClientError>),
    #[error("An error occurred in a `wl_surface`")]
    WlSurfaceError(#[source] Box<WlSurfaceError>),
    #[error("An error occurred in a `wl_compositor`")]
    WlCompositorError(#[source] Box<WlCompositorError>),
    #[error("An error occurred in a `wl_shm`")]
    WlShmError(#[source] Box<WlShmError>),
    #[error("An error occurred in a `wl_shm_pool`")]
    WlShmPoolError(#[source] Box<WlShmPoolError>),
    #[error("An error occurred in a `wl_region`")]
    WlRegionError(#[source] Box<WlRegionError>),
    #[error("An error occurred in a `wl_subsurface`")]
    WlSubsurfaceError(#[source] Box<WlSubsurfaceError>),
    #[error("An error occurred in a `wl_subcompositor`")]
    WlSubcompositorError(#[source] Box<WlSubcompositorError>),
    #[error("An error occurred in a `xdg_surface`")]
    XdgSurfaceError(#[source] Box<XdgSurfaceError>),
    #[error("An error occurred in a `xdg_positioner`")]
    XdgPositionerError(#[source] Box<XdgPositionerError>),
    #[error("An error occurred in a `xdg_popup`")]
    XdgPopupError(#[source] Box<XdgPopupError>),
    #[error("An error occurred in a `xdg_toplevel`")]
    XdgToplevelError(#[source] Box<XdgToplevelError>),
    #[error("An error occurred in a `xdg_wm_base`")]
    XdgWmBaseError(#[source] Box<XdgWmBaseError>),
    #[error("An error occurred in a `wl_buffer`")]
    WlBufferError(#[source] Box<WlBufferError>),
    #[error("An error occurred in a `wl_output`")]
    WlOutputError(#[source] Box<WlOutputError>),
    #[error("An error occurred in a `wl_seat`")]
    WlSeatError(#[source] Box<WlSeatError>),
    #[error("An error occurred in a `wl_pointer`")]
    WlPointerError(#[source] Box<WlPointerError>),
    #[error("An error occurred in a `wl_keyboard`")]
    WlKeyboardError(#[source] Box<WlKeyboardError>),
    #[error("An error occurred in a `wl_touch`")]
    WlTouchError(#[source] Box<WlTouchError>),
    #[error("Object {0} is not a display")]
    NotADisplay(ObjectId),
    #[error("An error occurred in a `wl_data_device`")]
    WlDataDeviceError(#[source] Box<WlDataDeviceError>),
    #[error("An error occurred in a `wl_data_device_manager`")]
    WlDataDeviceManagerError(#[source] Box<WlDataDeviceManagerError>),
    #[error("An error occurred in a `wl_data_offer`")]
    WlDataOfferError(#[source] Box<WlDataOfferError>),
    #[error("An error occurred in a `wl_data_source`")]
    WlDataSourceError(#[source] Box<WlDataSourceError>),
    #[error("An error occurred in a `zwp_linx_dmabuf_v1`")]
    ZwpLinuxDmabufV1Error(#[source] Box<ZwpLinuxDmabufV1Error>),
    #[error("An error occurred in a `zwp_linx_buffer_params_v1`")]
    ZwpLinuxBufferParamsV1Error(#[source] Box<ZwpLinuxBufferParamsV1Error>),
    #[error("An error occurred in a `wl_drm`")]
    WlDrmError(#[source] Box<WlDrmError>),
    #[error("An error occurred in a `zxdg_decoration_manager_v1`")]
    ZxdgDecorationManagerV1Error(#[source] Box<ZxdgDecorationManagerV1Error>),
    #[error("An error occurred in a `zxdg_toplevel_decoration_v1`")]
    ZxdgToplevelDecorationV1Error(#[source] Box<ZxdgToplevelDecorationV1Error>),
}

efrom!(ClientError, ParserError, MsgParserError);
efrom!(ClientError, WlDisplayError);
efrom!(ClientError, WlRegistryError);
efrom!(ClientError, WlSurfaceError);
efrom!(ClientError, WlCompositorError);
efrom!(ClientError, WlShmError);
efrom!(ClientError, WlShmPoolError);
efrom!(ClientError, WlRegionError);
efrom!(ClientError, WlSubsurfaceError);
efrom!(ClientError, WlSubcompositorError);
efrom!(ClientError, XdgSurfaceError);
efrom!(ClientError, XdgPositionerError);
efrom!(ClientError, XdgWmBaseError);
efrom!(ClientError, XdgToplevelError);
efrom!(ClientError, XdgPopupError);
efrom!(ClientError, WlBufferError);
efrom!(ClientError, WlOutputError);
efrom!(ClientError, WlSeatError);
efrom!(ClientError, WlTouchError);
efrom!(ClientError, WlPointerError);
efrom!(ClientError, WlKeyboardError);
efrom!(ClientError, WlDataDeviceManagerError);
efrom!(ClientError, WlDataDeviceError);
efrom!(ClientError, WlDataSourceError);
efrom!(ClientError, WlDataOfferError);
efrom!(ClientError, ZwpLinuxDmabufV1Error);
efrom!(ClientError, ZwpLinuxBufferParamsV1Error);
efrom!(ClientError, WlDrmError);
efrom!(ClientError, ZxdgDecorationManagerV1Error);
efrom!(ClientError, ZxdgToplevelDecorationV1Error);

impl ClientError {
    pub fn peer_closed(&self) -> bool {
        matches!(self, ClientError::Io(BufFdError::Closed))
    }
}
