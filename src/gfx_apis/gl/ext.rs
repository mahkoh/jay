use {
    crate::gfx_apis::gl::{
        RenderError,
        egl::sys::{EGL_EXTENSIONS, EGLDisplay},
        gl::sys::GL_EXTENSIONS,
        proc::ExtProc,
        sys::{EGL, EGL_DEVICE_EXT, EGL_TRUE, EGLDeviceEXT, GLESV2},
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
        if name.len() > 0
            && let Ok(s) = str::from_utf8(name)
        {
            res.insert(s.to_string());
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
        EXT_CLIENT_EXTENSION,
        EXT_PLATFORM_BASE,
        KHR_PLATFORM_GBM,
        KHR_DEBUG,
        EXT_DEVICE_QUERY,
}

pub fn get_client_ext() -> ClientExt {
    let map = [
        ("EGL_EXT_platform_base", EXT_PLATFORM_BASE),
        ("EGL_KHR_platform_gbm", KHR_PLATFORM_GBM),
        ("EGL_KHR_debug", KHR_DEBUG),
        ("EGL_EXT_device_query", EXT_DEVICE_QUERY),
    ];
    match unsafe { get_dpy_extensions(EGLDisplay::none()) } {
        Some(exts) => get_typed_ext(&exts, EXT_CLIENT_EXTENSION, &map),
        _ => ClientExt::none(),
    }
}

bitflags! {
    DisplayExt: u32;
        KHR_IMAGE_BASE,
        EXT_IMAGE_DMA_BUF_IMPORT,
        EXT_IMAGE_DMA_BUF_IMPORT_MODIFIERS,
        KHR_NO_CONFIG_CONTEXT,
        MESA_CONFIGLESS_CONTEXT,
        KHR_SURFACELESS_CONTEXT,
        IMG_CONTEXT_PRIORITY,
        EXT_CREATE_CONTEXT_ROBUSTNESS,
        KHR_FENCE_SYNC,
        KHR_WAIT_SYNC,
        ANDROID_NATIVE_FENCE_SYNC,
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
        GL_OES_EGL_IMAGE,
        GL_OES_EGL_IMAGE_EXTERNAL,
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

bitflags! {
    DevExt: u32;
        MESA_DEVICE_SOFTWARE,
}

pub fn get_device_ext(procs: &ExtProc, dpy: EGLDisplay) -> Result<DevExt, RenderError> {
    let map = [("EGL_MESA_device_software", MESA_DEVICE_SOFTWARE)];
    unsafe {
        let mut device = 0;
        if procs.eglQueryDisplayAttribEXT(dpy, EGL_DEVICE_EXT, &mut device) != EGL_TRUE {
            return Err(RenderError::QueryDisplayDevice);
        }
        let device = EGLDeviceEXT(device as _);
        let ext = procs.eglQueryDeviceStringEXT(device, EGL_EXTENSIONS);
        match get_extensions(ext) {
            Some(exts) => Ok(get_typed_ext(&exts, DevExt::none(), &map)),
            _ => Ok(DevExt::none()),
        }
    }
}
