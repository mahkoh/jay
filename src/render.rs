macro_rules! egl_transparent {
    ($name:ident) => {
        #[derive(Copy, Clone, Debug, Eq, PartialEq)]
        #[repr(transparent)]
        pub struct $name(pub *mut u8);

        impl $name {
            #[allow(dead_code)]
            pub const fn none() -> Self {
                Self(std::ptr::null_mut())
            }

            #[allow(dead_code)]
            pub fn is_none(self) -> bool {
                self.0.is_null()
            }
        }
    };
}

pub use renderer::*;
use {
    crate::video::{drm::DrmError, gbm::GbmError},
    thiserror::Error,
};

mod egl;
mod ext;
mod gl;
mod proc;
mod renderer;

pub mod sys {
    pub use super::{egl::sys::*, gl::sys::*};
}

pub fn init() -> Result<(), RenderError> {
    egl::init()
}

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("EGL library does not support `EGL_EXT_platform_base`")]
    ExtPlatformBase,
    #[error("Could not compile a shader")]
    ShaderCompileFailed,
    #[error("Could not link a program")]
    ProgramLink,
    #[error("Could not bind to `EGL_OPENGL_ES_API`")]
    BindFailed,
    #[error("EGL library does not support the GBM platform")]
    GbmExt,
    #[error("Could not create a GBM device")]
    Gbm(#[source] GbmError),
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
    #[error("The GLES driver does not support the XRGB8888 format")]
    XRGB888,
    #[error("The DRM device does not have a render node")]
    NoRenderNode,
}
