use {
    crate::{
        gfx_apis::gl::{
            egl::sys::{eglQueryString, EGLDisplay, EGL_EXTENSIONS},
            gl::sys::{glGetString, GL_EXTENSIONS},
        },
        utils::trim::AsciiTrim,
    },
    ahash::AHashSet,
    bstr::ByteSlice,
    std::{ffi::CStr, ops::BitOrAssign, str},
    uapi::c,
};

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
    #[derive(Copy, Clone, Debug)]
    pub struct ClientExt: u32 {
        const EXT_CLIENT_EXTENSION   = 1 << 0;
        const EXT_PLATFORM_BASE      = 1 << 1;
        const KHR_PLATFORM_GBM       = 1 << 2;
        const KHR_DEBUG              = 1 << 3;
    }
}

pub fn get_client_ext() -> ClientExt {
    let map = [
        ("EGL_EXT_platform_base", ClientExt::EXT_PLATFORM_BASE),
        ("EGL_KHR_platform_gbm", ClientExt::KHR_PLATFORM_GBM),
        ("EGL_KHR_debug", ClientExt::KHR_DEBUG),
    ];
    match unsafe { get_dpy_extensions(EGLDisplay::none()) } {
        Some(exts) => get_typed_ext(&exts, ClientExt::EXT_CLIENT_EXTENSION, &map),
        _ => ClientExt::empty(),
    }
}

bitflags::bitflags! {
    #[derive(Copy, Clone, Debug)]
    pub struct DisplayExt: u32 {
        const KHR_IMAGE_BASE                     = 1 << 0;
        const EXT_IMAGE_DMA_BUF_IMPORT           = 1 << 1;
        const EXT_IMAGE_DMA_BUF_IMPORT_MODIFIERS = 1 << 2;
        const KHR_NO_CONFIG_CONTEXT              = 1 << 3;
        const MESA_CONFIGLESS_CONTEXT            = 1 << 4;
        const KHR_SURFACELESS_CONTEXT            = 1 << 5;
        const IMG_CONTEXT_PRIORITY               = 1 << 6;
        const EXT_CREATE_CONTEXT_ROBUSTNESS      = 1 << 7;
    }
}

pub(crate) unsafe fn get_display_ext(dpy: EGLDisplay) -> DisplayExt {
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
        (
            "EGL_EXT_create_context_robustness",
            DisplayExt::EXT_CREATE_CONTEXT_ROBUSTNESS,
        ),
    ];
    match get_dpy_extensions(dpy) {
        Some(exts) => get_typed_ext(&exts, DisplayExt::empty(), &map),
        _ => DisplayExt::empty(),
    }
}

bitflags::bitflags! {
    #[derive(Copy, Clone, Debug)]
    pub struct GlExt: u32 {
        const GL_OES_EGL_IMAGE   = 1 << 0;
        const GL_OES_EGL_IMAGE_EXTERNAL   = 1 << 1;
    }
}

pub fn get_gl_ext() -> GlExt {
    let map = [
        ("GL_OES_EGL_image", GlExt::GL_OES_EGL_IMAGE),
        (
            "GL_OES_EGL_image_external",
            GlExt::GL_OES_EGL_IMAGE_EXTERNAL,
        ),
    ];
    match unsafe { get_extensions(glGetString(GL_EXTENSIONS) as _) } {
        Some(exts) => get_typed_ext(&exts, GlExt::empty(), &map),
        _ => GlExt::empty(),
    }
}
