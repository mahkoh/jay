use crate::gfx_apis::gl::RenderError;
use crate::gfx_apis::gl::egl::sys::EGL_DEBUG_MSG_CRITICAL_KHR;
use crate::gfx_apis::gl::egl::sys::EGL_DEBUG_MSG_ERROR_KHR;
use crate::gfx_apis::gl::egl::sys::EGL_DEBUG_MSG_INFO_KHR;
use crate::gfx_apis::gl::egl::sys::EGL_DEBUG_MSG_WARN_KHR;
use crate::gfx_apis::gl::egl::sys::EGL_NONE;
use crate::gfx_apis::gl::egl::sys::EGL_OPENGL_ES_API;
use crate::gfx_apis::gl::egl::sys::EGL_TRUE;
use crate::gfx_apis::gl::egl::sys::EGLAttrib;
use crate::gfx_apis::gl::egl::sys::EGLLabelKHR;
use crate::gfx_apis::gl::egl::sys::EGLenum;
use crate::gfx_apis::gl::egl::sys::EGLint;
use crate::gfx_apis::gl::ext::ClientExt;
use crate::gfx_apis::gl::ext::EXT_PLATFORM_BASE;
use crate::gfx_apis::gl::ext::KHR_DEBUG;
use crate::gfx_apis::gl::ext::KHR_PLATFORM_GBM;
use crate::gfx_apis::gl::ext::get_client_ext;
use crate::gfx_apis::gl::proc::ExtProc;
use crate::gfx_apis::gl::sys::EGL;
use bstr::ByteSlice;
use log::Level;
use std::ffi::CStr;
use std::sync::LazyLock;
use sys::EGL_BAD_ACCESS;
use sys::EGL_BAD_ALLOC;
use sys::EGL_BAD_ATTRIBUTE;
use sys::EGL_BAD_CONFIG;
use sys::EGL_BAD_CONTEXT;
use sys::EGL_BAD_CURRENT_SURFACE;
use sys::EGL_BAD_DEVICE_EXT;
use sys::EGL_BAD_DISPLAY;
use sys::EGL_BAD_MATCH;
use sys::EGL_BAD_NATIVE_PIXMAP;
use sys::EGL_BAD_NATIVE_WINDOW;
use sys::EGL_BAD_PARAMETER;
use sys::EGL_BAD_SURFACE;
use sys::EGL_CONTEXT_LOST;
use sys::EGL_NOT_INITIALIZED;
use sys::EGL_SUCCESS;
use uapi::c;

pub mod context;
pub mod display;
pub mod image;
pub mod sys;

pub(crate) static PROCS: LazyLock<Option<ExtProc>> = LazyLock::new(ExtProc::load);

pub(crate) static EXTS: LazyLock<ClientExt> = LazyLock::new(get_client_ext);

pub(in crate::gfx_apis::gl) fn init() -> Result<(), RenderError> {
    let Some(egl) = EGL.as_ref() else {
        return Err(RenderError::LoadEgl);
    };
    let Some(procs) = PROCS.as_ref() else {
        return Err(RenderError::LoadEglProcs);
    };
    if !EXTS.contains(EXT_PLATFORM_BASE) {
        return Err(RenderError::ExtPlatformBase);
    }
    if !EXTS.contains(KHR_PLATFORM_GBM) {
        return Err(RenderError::GbmExt);
    }
    if EXTS.contains(KHR_DEBUG) {
        let attrib: &[EGLAttrib] = &[
            EGL_DEBUG_MSG_CRITICAL_KHR as _,
            EGL_TRUE as _,
            EGL_DEBUG_MSG_ERROR_KHR as _,
            EGL_TRUE as _,
            EGL_DEBUG_MSG_WARN_KHR as _,
            EGL_TRUE as _,
            EGL_DEBUG_MSG_INFO_KHR as _,
            EGL_TRUE as _,
            EGL_NONE as _,
        ];
        unsafe {
            procs.eglDebugMessageControlKHR(egl_log, attrib.as_ptr());
        }
    }
    if unsafe { (egl.eglBindAPI)(EGL_OPENGL_ES_API) } != EGL_TRUE {
        return Err(RenderError::BindFailed);
    }
    Ok(())
}

unsafe extern "C" fn egl_log(
    error: EGLenum,
    command: *const c::c_char,
    message_type: EGLint,
    _thread_label: EGLLabelKHR,
    _object_label: EGLLabelKHR,
    message: *const c::c_char,
) {
    let level = match message_type {
        EGL_DEBUG_MSG_CRITICAL_KHR => Level::Error,
        EGL_DEBUG_MSG_ERROR_KHR => Level::Error,
        EGL_DEBUG_MSG_WARN_KHR => Level::Warn,
        EGL_DEBUG_MSG_INFO_KHR => Level::Info,
        _ => Level::Warn,
    };
    let command = if !command.is_null() {
        unsafe { CStr::from_ptr(command).to_bytes() }
    } else {
        b"none"
    };
    let message = if !message.is_null() {
        unsafe { CStr::from_ptr(message).to_bytes() }
    } else {
        b"none"
    };
    let err_name = error_name(error);
    log::log!(
        level,
        "EGL: command: {}, error: {} (0x{:x}), message: {}",
        command.as_bstr(),
        err_name,
        error,
        message.as_bstr()
    );
}

fn error_name(error: EGLenum) -> &'static str {
    macro_rules! en {
        ($($name:ident,)*) => {
            match error as _ {
                $($name => stringify!($name),)*
                _ => "unknown",
            }
        }
    }
    en! {
        EGL_SUCCESS,
        EGL_NOT_INITIALIZED,
        EGL_BAD_ACCESS,
        EGL_BAD_ALLOC,
        EGL_BAD_ATTRIBUTE,
        EGL_BAD_CONTEXT,
        EGL_BAD_CONFIG,
        EGL_BAD_CURRENT_SURFACE,
        EGL_BAD_DISPLAY,
        EGL_BAD_DEVICE_EXT,
        EGL_BAD_SURFACE,
        EGL_BAD_MATCH,
        EGL_BAD_PARAMETER,
        EGL_BAD_NATIVE_PIXMAP,
        EGL_BAD_NATIVE_WINDOW,
        EGL_CONTEXT_LOST,
    }
}
