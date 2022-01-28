use crate::drm::drm::Drm;
use crate::render::egl::device::EglDevice;
use crate::render::egl::sys::{
    eglBindAPI, EGLAttrib, EGLLabelKHR, EGLenum, EGLint, EGL_DEBUG_MSG_CRITICAL_KHR,
    EGL_DEBUG_MSG_ERROR_KHR, EGL_DEBUG_MSG_INFO_KHR, EGL_DEBUG_MSG_WARN_KHR,
    EGL_DRM_DEVICE_FILE_EXT, EGL_NONE, EGL_OPENGL_ES_API, EGL_TRUE,
};
use crate::render::ext::{get_client_ext, get_device_ext, ClientExt, DeviceExt};
use crate::render::proc::ExtProc;
use crate::render::RenderError;
use bstr::ByteSlice;
use log::Level;
use once_cell::sync::Lazy;
use std::ffi::CStr;
use std::ptr;
use sys::{
    EGL_BAD_ACCESS, EGL_BAD_ALLOC, EGL_BAD_ATTRIBUTE, EGL_BAD_CONFIG, EGL_BAD_CONTEXT,
    EGL_BAD_CURRENT_SURFACE, EGL_BAD_DEVICE_EXT, EGL_BAD_DISPLAY, EGL_BAD_MATCH,
    EGL_BAD_NATIVE_PIXMAP, EGL_BAD_NATIVE_WINDOW, EGL_BAD_PARAMETER, EGL_BAD_SURFACE,
    EGL_CONTEXT_LOST, EGL_NOT_INITIALIZED, EGL_SUCCESS,
};
use uapi::c;

pub mod context;
pub mod device;
pub mod display;
pub mod image;
pub mod sys;

pub(super) static PROCS: Lazy<ExtProc> = Lazy::new(|| ExtProc::load());

pub(super) static EXTS: Lazy<ClientExt> = Lazy::new(|| get_client_ext());

pub fn init() -> Result<(), RenderError> {
    if !EXTS.contains(ClientExt::EXT_PLATFORM_BASE) {
        return Err(RenderError::ExtPlatformBase);
    }
    if !EXTS.device_query() {
        return Err(RenderError::DeviceQuery);
    }
    if !EXTS.device_enumeration() {
        return Err(RenderError::DeviceEnumeration);
    }
    if EXTS.contains(ClientExt::KHR_DEBUG) {
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
            PROCS.eglDebugMessageControlKHR(egl_log, attrib.as_ptr());
        }
    }
    if unsafe { eglBindAPI(EGL_OPENGL_ES_API) } != EGL_TRUE {
        return Err(RenderError::BindFailed);
    }
    Ok(())
}

pub fn find_drm_device(drm: &Drm) -> Result<Option<EglDevice>, RenderError> {
    let drm_dev = drm.get_device()?;
    for device in query_devices()? {
        if device.exts.contains(DeviceExt::EXT_DEVICE_DRM) {
            let device_file = device.query_string(EGL_DRM_DEVICE_FILE_EXT)?;
            for (_, name) in drm_dev.nodes() {
                if device_file == name {
                    return Ok(Some(device));
                }
            }
        }
    }
    Ok(None)
}

pub fn query_devices() -> Result<Vec<EglDevice>, RenderError> {
    if !EXTS.device_enumeration() {
        return Err(RenderError::DeviceEnumeration);
    }
    unsafe {
        let mut devices = vec![];
        let mut num_devices = 0;
        let res = PROCS.eglQueryDevicesEXT(num_devices, ptr::null_mut(), &mut num_devices);
        if res != EGL_TRUE {
            return Err(RenderError::QueryDevices);
        }
        devices.reserve_exact(num_devices as usize);
        let res = PROCS.eglQueryDevicesEXT(num_devices, devices.as_mut_ptr(), &mut num_devices);
        if res != EGL_TRUE {
            return Err(RenderError::QueryDevices);
        }
        devices.set_len(num_devices as usize);
        Ok(devices
            .into_iter()
            .map(|d| EglDevice {
                exts: get_device_ext(d),
                dev: d,
            })
            .collect())
    }
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
        CStr::from_ptr(command).to_bytes()
    } else {
        b"none"
    };
    let message = if !message.is_null() {
        CStr::from_ptr(message).to_bytes()
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
