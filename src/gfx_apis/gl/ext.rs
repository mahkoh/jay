use {
    crate::gfx_apis::gl::{
        egl::sys::{EGLDisplay, EGL_EXTENSIONS},
        gl::sys::GL_EXTENSIONS,
        sys::{EGL, GLESV2},
        RenderError,
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
    let ext = unsafe { CStr::from_ptr(ext).to_bytes() };
    for part in ext.split_str(" ") {
        let name = part.trim_ascii();
        if name.len() > 0 {
            if let Ok(s) = str::from_utf8(name) {
                res.insert(s.to_string());
            }
        }
    }
    Some(res)
}

unsafe fn get_dpy_extensions(dpy: EGLDisplay) -> Option<AHashSet<String>> {
    unsafe {
        let ext = (EGL.as_ref()?.eglQueryString)(dpy, EGL_EXTENSIONS);
        get_extensions(ext)
    }
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

bitflags! {
    ClientExt: u32;
        EXT_CLIENT_EXTENSION   = 1 << 0,
        EXT_PLATFORM_BASE      = 1 << 1,
        KHR_PLATFORM_GBM       = 1 << 2,
        KHR_DEBUG              = 1 << 3,
}

pub fn get_client_ext() -> ClientExt {
    let map = [
        ("EGL_EXT_platform_base", EXT_PLATFORM_BASE),
        ("EGL_KHR_platform_gbm", KHR_PLATFORM_GBM),
        ("EGL_KHR_debug", KHR_DEBUG),
    ];
    match unsafe { get_dpy_extensions(EGLDisplay::none()) } {
        Some(exts) => get_typed_ext(&exts, EXT_CLIENT_EXTENSION, &map),
        _ => ClientExt::none(),
    }
}

bitflags! {
    DisplayExt: u32;
        KHR_IMAGE_BASE                     = 1 << 0,
        EXT_IMAGE_DMA_BUF_IMPORT           = 1 << 1,
        EXT_IMAGE_DMA_BUF_IMPORT_MODIFIERS = 1 << 2,
        KHR_NO_CONFIG_CONTEXT              = 1 << 3,
        MESA_CONFIGLESS_CONTEXT            = 1 << 4,
        KHR_SURFACELESS_CONTEXT            = 1 << 5,
        IMG_CONTEXT_PRIORITY               = 1 << 6,
        EXT_CREATE_CONTEXT_ROBUSTNESS      = 1 << 7,
        KHR_FENCE_SYNC                     = 1 << 8,
        KHR_WAIT_SYNC                      = 1 << 9,
        ANDROID_NATIVE_FENCE_SYNC          = 1 << 10,
}

pub(crate) unsafe fn get_display_ext(dpy: EGLDisplay) -> DisplayExt {
    let map = [
        ("EGL_KHR_image_base", KHR_IMAGE_BASE),
        ("EGL_EXT_image_dma_buf_import", EXT_IMAGE_DMA_BUF_IMPORT),
        (
            "EGL_EXT_image_dma_buf_import_modifiers",
            EXT_IMAGE_DMA_BUF_IMPORT_MODIFIERS,
        ),
        ("EGL_KHR_no_config_context", KHR_NO_CONFIG_CONTEXT),
        ("EGL_MESA_configless_context", MESA_CONFIGLESS_CONTEXT),
        ("EGL_KHR_surfaceless_context", KHR_SURFACELESS_CONTEXT),
        ("EGL_IMG_context_priority", IMG_CONTEXT_PRIORITY),
        (
            "EGL_EXT_create_context_robustness",
            EXT_CREATE_CONTEXT_ROBUSTNESS,
        ),
        ("EGL_KHR_fence_sync", KHR_FENCE_SYNC),
        ("EGL_KHR_wait_sync", KHR_WAIT_SYNC),
        ("EGL_ANDROID_native_fence_sync", ANDROID_NATIVE_FENCE_SYNC),
    ];
    match unsafe { get_dpy_extensions(dpy) } {
        Some(exts) => get_typed_ext(&exts, DisplayExt::none(), &map),
        _ => DisplayExt::none(),
    }
}

bitflags! {
    GlExt: u32;
        GL_OES_EGL_IMAGE          = 1 << 0,
        GL_OES_EGL_IMAGE_EXTERNAL = 1 << 1,
}

pub fn get_gl_ext() -> Result<GlExt, RenderError> {
    let map = [
        ("GL_OES_EGL_image", GL_OES_EGL_IMAGE),
        ("GL_OES_EGL_image_external", GL_OES_EGL_IMAGE_EXTERNAL),
    ];
    let Some(gles) = GLESV2.as_ref() else {
        return Err(RenderError::LoadGlesV2);
    };
    match unsafe { get_extensions((gles.glGetString)(GL_EXTENSIONS) as _) } {
        Some(exts) => Ok(get_typed_ext(&exts, GlExt::none(), &map)),
        _ => Ok(GlExt::none()),
    }
}
