use crate::gles2::egl::PROCS;
use crate::gles2::sys::{
    eglQueryString, glGetString, EGLDeviceEXT, EGLDisplay, EGL_EXTENSIONS, GL_EXTENSIONS,
};
use ahash::AHashSet;
use bstr::ByteSlice;
use std::ffi::CStr;
use std::ops::BitOrAssign;
use std::str;
use uapi::c;

unsafe fn get_extensions(ext: *const c::c_char) -> Option<AHashSet<String>> {
    if ext.is_null() {
        return None;
    }
    let mut res = AHashSet::new();
    let ext = CStr::from_ptr(ext).to_bytes();
    for part in ext.split_str(" ") {
        let name = part.trim();
        if name.len() > 0 {
            if let Ok(s) = str::from_utf8(name) {
                res.insert(s.to_string());
            }
        }
    }
    Some(res)
}

unsafe fn get_dpy_extensions(dpy: EGLDisplay) -> Option<AHashSet<String>> {
    let ext = eglQueryString(dpy, EGL_EXTENSIONS);
    get_extensions(ext)
}

fn get_typed_ext<T>(exts: &AHashSet<String>, mut base: T, map: &[(&str, T)]) -> T
where
    T: BitOrAssign + Copy,
{
    for (name, ext) in map.iter().copied() {
        if exts.contains(name) {
            base |= ext;
        }
    }
    base
}

bitflags::bitflags! {
    pub struct ClientExt: u32 {
        const EXT_CLIENT_EXTENSION   = 1 << 0;
        const EXT_PLATFORM_BASE      = 1 << 1;
        const KHR_PLATFORM_GBM       = 1 << 2;
        const EXT_PLATFORM_DEVICE    = 1 << 3;
        const EXT_DEVICE_BASE        = 1 << 4;
        const EXT_DEVICE_ENUMERATION = 1 << 5;
        const EXT_DEVICE_QUERY       = 1 << 6;
        const KHR_DEBUG              = 1 << 7;
    }
}

impl ClientExt {
    pub fn device_enumeration(self) -> bool {
        self.intersects(Self::EXT_DEVICE_BASE | Self::EXT_DEVICE_ENUMERATION)
    }

    pub fn device_query(self) -> bool {
        self.intersects(Self::EXT_DEVICE_BASE | Self::EXT_DEVICE_QUERY)
    }
}

pub fn get_client_ext() -> ClientExt {
    let map = [
        ("EGL_EXT_platform_base", ClientExt::EXT_PLATFORM_BASE),
        ("EGL_KHR_platform_gbm", ClientExt::KHR_PLATFORM_GBM),
        ("EGL_EXT_platform_device", ClientExt::EXT_PLATFORM_DEVICE),
        ("EGL_EXT_device_base", ClientExt::EXT_DEVICE_BASE),
        (
            "EGL_EXT_device_enumeration",
            ClientExt::EXT_DEVICE_ENUMERATION,
        ),
        ("EGL_EXT_device_query", ClientExt::EXT_DEVICE_QUERY),
        ("EGL_KHR_debug", ClientExt::KHR_DEBUG),
    ];
    match unsafe { get_dpy_extensions(EGLDisplay::none()) } {
        Some(exts) => get_typed_ext(&exts, ClientExt::EXT_CLIENT_EXTENSION, &map),
        _ => ClientExt::empty(),
    }
}

bitflags::bitflags! {
    pub struct DisplayExt: u32 {
        const KHR_IMAGE_BASE                     = 1 << 0;
        const EXT_IMAGE_DMA_BUF_IMPORT           = 1 << 1;
        const EXT_IMAGE_DMA_BUF_IMPORT_MODIFIERS = 1 << 2;
        const KHR_NO_CONFIG_CONTEXT              = 1 << 3;
        const MESA_CONFIGLESS_CONTEXT            = 1 << 4;
        const KHR_SURFACELESS_CONTEXT            = 1 << 5;
        const IMG_CONTEXT_PRIORITY               = 1 << 6;
    }
}

pub(super) unsafe fn get_display_ext(dpy: EGLDisplay) -> DisplayExt {
    let map = [
        ("EGL_KHR_image_base", DisplayExt::KHR_IMAGE_BASE),
        (
            "EGL_EXT_image_dma_buf_import",
            DisplayExt::EXT_IMAGE_DMA_BUF_IMPORT,
        ),
        (
            "EGL_EXT_image_dma_buf_import_modifiers",
            DisplayExt::EXT_IMAGE_DMA_BUF_IMPORT_MODIFIERS,
        ),
        (
            "EGL_KHR_no_config_context",
            DisplayExt::KHR_NO_CONFIG_CONTEXT,
        ),
        (
            "EGL_MESA_configless_context",
            DisplayExt::MESA_CONFIGLESS_CONTEXT,
        ),
        (
            "EGL_KHR_surfaceless_context",
            DisplayExt::KHR_SURFACELESS_CONTEXT,
        ),
        ("EGL_IMG_context_priority", DisplayExt::IMG_CONTEXT_PRIORITY),
    ];
    match get_dpy_extensions(dpy) {
        Some(exts) => get_typed_ext(&exts, DisplayExt::empty(), &map),
        _ => DisplayExt::empty(),
    }
}

bitflags::bitflags! {
    pub struct DeviceExt: u32 {
        const MESA_DEVICE_SOFTWARE       = 1 << 0;
        const EXT_DEVICE_PERSISTENT_ID   = 1 << 1;
        const EXT_DEVICE_DRM             = 1 << 2;
        const EXT_DEVICE_DRM_RENDER_NODE = 1 << 3;
    }
}

pub(super) unsafe fn get_device_ext(dev: EGLDeviceEXT) -> DeviceExt {
    let map = [
        ("EGL_MESA_device_software", DeviceExt::MESA_DEVICE_SOFTWARE),
        (
            "EGL_EXT_device_persistent_id",
            DeviceExt::EXT_DEVICE_PERSISTENT_ID,
        ),
        ("EGL_EXT_device_drm", DeviceExt::EXT_DEVICE_DRM),
        (
            "EGL_EXT_device_drm_render_node",
            DeviceExt::EXT_DEVICE_DRM_RENDER_NODE,
        ),
    ];
    let ext = PROCS.eglQueryDeviceStringEXT(dev, EGL_EXTENSIONS);
    match get_extensions(ext) {
        Some(exts) => get_typed_ext(&exts, DeviceExt::empty(), &map),
        _ => DeviceExt::empty(),
    }
}

bitflags::bitflags! {
    pub struct GlExt: u32 {
        const GL_OES_EGL_IMAGE   = 1 << 0;
    }
}

pub fn get_gl_ext() -> GlExt {
    let map = [("GL_OES_EGL_image", GlExt::GL_OES_EGL_IMAGE)];
    match unsafe { get_extensions(glGetString(GL_EXTENSIONS) as _) } {
        Some(exts) => get_typed_ext(&exts, GlExt::empty(), &map),
        _ => GlExt::empty(),
    }
}
