pub mod egl;
pub mod ext;
mod ext_proc;
pub mod gl;
pub mod sys;

use crate::drm::drm::DrmError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GlesError {
    #[error("EGL library does not support `EGL_EXT_platform_base`")]
    ExtPlatformBase,
    #[error("Could not compile a shader")]
    ShaderCompileFailed,
    #[error("Could not link a program")]
    ProgramLink,
    #[error("Could not bind to `EGL_OPENGL_ES_API`")]
    BindFailed,
    #[error("EGL library does not support device enumeration")]
    DeviceEnumeration,
    #[error("EGL library does not support device querying")]
    DeviceQuery,
    #[error("`eglQueryDeviceStringEXT` failed")]
    DeviceQueryString,
    #[error("`eglCreateContext` failed")]
    CreateContext,
    #[error("`eglMakeCurrent` failed")]
    MakeCurrent,
    #[error("`eglCreateImageKHR` failed")]
    CreateImage,
    #[error("Image buffer is too small")]
    SmallImageBuffer,
    #[error("Binding a renderbuffer to a framebuffer failed")]
    CreateFramebuffer,
    #[error("`eglQueryDevicesEXT` failed")]
    QueryDevices,
    #[error("`eglGetPlatformDisplayEXT` failed")]
    GetDisplay,
    #[error("`eglInitialize` failed")]
    Initialize,
    #[error("EGL display does not support `EGL_EXT_image_dma_buf_import_modifiers`")]
    DmaBufImport,
    #[error("GLES driver does not support `GL_OES_EGL_image`")]
    OesEglImage,
    #[error("EGL display does not support `EGL_KHR_image_base`")]
    ImageBase,
    #[error(
        "EGL display does not support `EGL_KHR_no_config_context` or `EGL_MESA_configless_context`"
    )]
    ConfiglessContext,
    #[error("EGL display does not support `EGL_KHR_surfaceless_context`")]
    SurfacelessContext,
    #[error("`eglQueryDmaBufFormatsEXT` failed")]
    QueryDmaBufFormats,
    #[error(transparent)]
    DrmError(#[from] DrmError),
}
