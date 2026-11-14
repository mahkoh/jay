use crate::backend::DrmDeviceId;
use crate::format::Format;
use crate::format::formats;
use crate::gfx_api::GfxFormat;
use crate::gfx_api::GfxWriteModifier;
use crate::gfx_apis::gl::RenderError;
use crate::gfx_apis::gl::egl::PROCS;
use crate::gfx_apis::gl::egl::context::EglContext;
use crate::gfx_apis::gl::egl::image::EglImage;
use crate::gfx_apis::gl::egl::sys::EGL_CONTEXT_CLIENT_VERSION;
use crate::gfx_apis::gl::egl::sys::EGL_DMA_BUF_PLANE0_FD_EXT;
use crate::gfx_apis::gl::egl::sys::EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT;
use crate::gfx_apis::gl::egl::sys::EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT;
use crate::gfx_apis::gl::egl::sys::EGL_DMA_BUF_PLANE0_OFFSET_EXT;
use crate::gfx_apis::gl::egl::sys::EGL_DMA_BUF_PLANE0_PITCH_EXT;
use crate::gfx_apis::gl::egl::sys::EGL_DMA_BUF_PLANE1_FD_EXT;
use crate::gfx_apis::gl::egl::sys::EGL_DMA_BUF_PLANE1_MODIFIER_HI_EXT;
use crate::gfx_apis::gl::egl::sys::EGL_DMA_BUF_PLANE1_MODIFIER_LO_EXT;
use crate::gfx_apis::gl::egl::sys::EGL_DMA_BUF_PLANE1_OFFSET_EXT;
use crate::gfx_apis::gl::egl::sys::EGL_DMA_BUF_PLANE1_PITCH_EXT;
use crate::gfx_apis::gl::egl::sys::EGL_DMA_BUF_PLANE2_FD_EXT;
use crate::gfx_apis::gl::egl::sys::EGL_DMA_BUF_PLANE2_MODIFIER_HI_EXT;
use crate::gfx_apis::gl::egl::sys::EGL_DMA_BUF_PLANE2_MODIFIER_LO_EXT;
use crate::gfx_apis::gl::egl::sys::EGL_DMA_BUF_PLANE2_OFFSET_EXT;
use crate::gfx_apis::gl::egl::sys::EGL_DMA_BUF_PLANE2_PITCH_EXT;
use crate::gfx_apis::gl::egl::sys::EGL_DMA_BUF_PLANE3_FD_EXT;
use crate::gfx_apis::gl::egl::sys::EGL_DMA_BUF_PLANE3_MODIFIER_HI_EXT;
use crate::gfx_apis::gl::egl::sys::EGL_DMA_BUF_PLANE3_MODIFIER_LO_EXT;
use crate::gfx_apis::gl::egl::sys::EGL_DMA_BUF_PLANE3_OFFSET_EXT;
use crate::gfx_apis::gl::egl::sys::EGL_DMA_BUF_PLANE3_PITCH_EXT;
use crate::gfx_apis::gl::egl::sys::EGL_DRM_DEVICE_FILE_EXT;
use crate::gfx_apis::gl::egl::sys::EGL_DRM_RENDER_NODE_FILE_EXT;
use crate::gfx_apis::gl::egl::sys::EGL_HEIGHT;
use crate::gfx_apis::gl::egl::sys::EGL_IMAGE_PRESERVED_KHR;
use crate::gfx_apis::gl::egl::sys::EGL_LINUX_DMA_BUF_EXT;
use crate::gfx_apis::gl::egl::sys::EGL_LINUX_DRM_FOURCC_EXT;
use crate::gfx_apis::gl::egl::sys::EGL_NONE;
use crate::gfx_apis::gl::egl::sys::EGL_TRUE;
use crate::gfx_apis::gl::egl::sys::EGL_WIDTH;
use crate::gfx_apis::gl::egl::sys::EGLClientBuffer;
use crate::gfx_apis::gl::egl::sys::EGLConfig;
use crate::gfx_apis::gl::egl::sys::EGLContext;
use crate::gfx_apis::gl::egl::sys::EGLDeviceEXT;
use crate::gfx_apis::gl::egl::sys::EGLDisplay;
use crate::gfx_apis::gl::egl::sys::EGLint;
use crate::gfx_apis::gl::ext::ANDROID_NATIVE_FENCE_SYNC;
use crate::gfx_apis::gl::ext::DisplayExt;
use crate::gfx_apis::gl::ext::EXT_CREATE_CONTEXT_ROBUSTNESS;
use crate::gfx_apis::gl::ext::EXT_DEVICE_DRM;
use crate::gfx_apis::gl::ext::EXT_DEVICE_DRM_RENDER_NODE;
use crate::gfx_apis::gl::ext::EXT_IMAGE_DMA_BUF_IMPORT_MODIFIERS;
use crate::gfx_apis::gl::ext::GL_OES_EGL_IMAGE;
use crate::gfx_apis::gl::ext::GL_OES_EGL_IMAGE_EXTERNAL;
use crate::gfx_apis::gl::ext::GlExt;
use crate::gfx_apis::gl::ext::KHR_FENCE_SYNC;
use crate::gfx_apis::gl::ext::KHR_IMAGE_BASE;
use crate::gfx_apis::gl::ext::KHR_NO_CONFIG_CONTEXT;
use crate::gfx_apis::gl::ext::KHR_SURFACELESS_CONTEXT;
use crate::gfx_apis::gl::ext::KHR_WAIT_SYNC;
use crate::gfx_apis::gl::ext::MESA_CONFIGLESS_CONTEXT;
use crate::gfx_apis::gl::ext::MESA_DEVICE_SOFTWARE;
use crate::gfx_apis::gl::ext::get_device_ext;
use crate::gfx_apis::gl::ext::get_display_ext;
use crate::gfx_apis::gl::ext::get_gl_ext;
use crate::gfx_apis::gl::proc::ExtProc;
use crate::gfx_apis::gl::sys::EGL;
use crate::gfx_apis::gl::sys::EGL_CONTEXT_OPENGL_RESET_NOTIFICATION_STRATEGY_EXT;
use crate::gfx_apis::gl::sys::EGL_LOSE_CONTEXT_ON_RESET_EXT;
use crate::gfx_apis::gl::sys::EGL_PLATFORM_DEVICE_EXT;
use crate::gfx_apis::gl::sys::Egl;
use crate::gfx_apis::gl::sys::GLESV2;
use crate::gfx_apis::gl::sys::GlesV2;
use crate::utils::bhash::BHashMap;
use crate::utils::copyhashmap::CopyHashMap;
use crate::video::INVALID_MODIFIER;
use crate::video::Modifier;
use crate::video::dmabuf::DmaBuf;
use crate::video::drm::Drm;
use crate::video::gbm::GbmDevice;
use indexmap::IndexMap;
use indexmap::IndexSet;
use std::ffi::CStr;
use std::ptr;
use std::rc::Rc;
use std::rc::Weak;

#[derive(Debug)]
pub struct EglFormat {
    format: &'static Format,
    implicit_external_only: bool,
    modifiers: IndexMap<u64, EglModifier>,
}

#[derive(Debug)]
pub struct EglModifier {
    modifier: u64,
    external_only: bool,
}

#[derive(Debug)]
pub struct EglDisplay {
    pub drm_device_id: Option<DrmDeviceId>,
    pub egl: &'static Egl,
    pub gles: &'static GlesV2,
    pub procs: &'static ExtProc,
    pub exts: DisplayExt,
    pub formats: BHashMap<u32, EglFormat>,
    pub gbm: Rc<GbmDevice>,
    pub dpy: EGLDisplay,
    pub explicit_sync: bool,
    _holder: Rc<EglDisplayHolder>,
}

#[derive(Debug)]
struct EglDisplayHolder {
    egl: &'static Egl,
    dpy: EGLDisplay,
}

impl EglDisplay {
    pub(in crate::gfx_apis::gl) fn create(
        drm_device_id: Option<DrmDeviceId>,
        drm: &Drm,
        software: bool,
    ) -> Result<Rc<Self>, RenderError> {
        unsafe {
            let Some(egl) = EGL.as_ref() else {
                return Err(RenderError::LoadEgl);
            };
            let Some(gles) = GLESV2.as_ref() else {
                return Err(RenderError::LoadGlesV2);
            };
            let Some(procs) = PROCS.as_ref() else {
                return Err(RenderError::LoadEglProcs);
            };
            let gbm = match GbmDevice::new(drm) {
                Ok(gbm) => gbm,
                Err(e) => return Err(RenderError::Gbm(e)),
            };
            let holder = get_or_create_display(egl, procs, drm, software)?;
            let mut dpy = EglDisplay {
                drm_device_id,
                egl,
                gles,
                procs,
                exts: DisplayExt::none(),
                formats: BHashMap::default(),
                gbm: Rc::new(gbm),
                dpy: holder.dpy,
                explicit_sync: false,
                _holder: holder,
            };
            dpy.exts = get_display_ext(dpy.dpy);
            if !dpy.exts.intersects(KHR_IMAGE_BASE) {
                return Err(RenderError::ImageBase);
            }
            if !dpy.exts.intersects(EXT_IMAGE_DMA_BUF_IMPORT_MODIFIERS) {
                return Err(RenderError::DmaBufImport);
            }
            if !dpy
                .exts
                .intersects(KHR_NO_CONFIG_CONTEXT | MESA_CONFIGLESS_CONTEXT)
            {
                return Err(RenderError::ConfiglessContext);
            }
            if !dpy.exts.intersects(KHR_SURFACELESS_CONTEXT) {
                return Err(RenderError::SurfacelessContext);
            }
            dpy.formats = query_formats(procs, dpy.dpy)?;
            dpy.explicit_sync = dpy
                .exts
                .contains(KHR_FENCE_SYNC | KHR_WAIT_SYNC | ANDROID_NATIVE_FENCE_SYNC);

            if !dpy.explicit_sync {
                log::error!("Driver does not support explicit sync. Rendering will block.");
            }

            Ok(Rc::new(dpy))
        }
    }

    pub(in crate::gfx_apis::gl) fn create_context(
        self: &Rc<Self>,
    ) -> Result<Rc<EglContext>, RenderError> {
        let mut attrib = vec![EGL_CONTEXT_CLIENT_VERSION, 2];
        if self.exts.contains(EXT_CREATE_CONTEXT_ROBUSTNESS) {
            attrib.push(EGL_CONTEXT_OPENGL_RESET_NOTIFICATION_STRATEGY_EXT);
            attrib.push(EGL_LOSE_CONTEXT_ON_RESET_EXT);
        } else {
            log::warn!("EGL display does not support gpu reset notifications");
        }
        attrib.push(EGL_NONE);
        let ctx = unsafe {
            (self.egl.eglCreateContext)(
                self.dpy,
                EGLConfig::none(),
                EGLContext::none(),
                attrib.as_ptr(),
            )
        };
        if ctx.is_none() {
            return Err(RenderError::CreateContext);
        }
        let mut ctx = EglContext {
            dpy: self.clone(),
            ext: GlExt::none(),
            ctx,
            formats: Default::default(),
        };
        ctx.ext = ctx.with_current(get_gl_ext)?;
        if !ctx.ext.contains(GL_OES_EGL_IMAGE) {
            return Err(RenderError::OesEglImage);
        }
        ctx.formats = {
            let mut formats = BHashMap::default();
            let supports_external_only = ctx.ext.contains(GL_OES_EGL_IMAGE_EXTERNAL);
            for (&drm, format) in &self.formats {
                if format.implicit_external_only && !supports_external_only {
                    continue;
                }
                let mut read_modifiers = IndexSet::new();
                let mut write_modifiers = IndexMap::new();
                for modifier in format.modifiers.values() {
                    if modifier.external_only && !supports_external_only {
                        continue;
                    }
                    if !modifier.external_only {
                        write_modifiers.insert(
                            modifier.modifier,
                            GfxWriteModifier {
                                needs_render_usage: true,
                            },
                        );
                    }
                    read_modifiers.insert(modifier.modifier);
                }
                if !read_modifiers.is_empty()
                    || !write_modifiers.is_empty()
                    || format.format.shm_info.is_some()
                {
                    formats.insert(
                        drm,
                        GfxFormat {
                            format: format.format,
                            read_modifiers,
                            write_modifiers,
                            supports_shm: format.format.shm_info.is_some(),
                        },
                    );
                }
            }
            Rc::new(formats)
        };
        if ctx.formats.is_empty() {
            return Err(RenderError::NoSupportedFormats);
        }
        Ok(Rc::new(ctx))
    }

    pub(in crate::gfx_apis::gl) fn import_dmabuf(
        self: &Rc<Self>,
        buf: &Rc<DmaBuf>,
    ) -> Result<Rc<EglImage>, RenderError> {
        let format = match self.formats.get(&buf.format.drm) {
            Some(fmt) => match fmt.modifiers.get(&buf.modifier) {
                Some(fmt) => fmt,
                _ => return Err(RenderError::UnsupportedModifier),
            },
            _ => return Err(RenderError::UnsupportedFormat),
        };
        struct PlaneKey {
            fd: EGLint,
            offset: EGLint,
            pitch: EGLint,
            mod_lo: EGLint,
            mod_hi: EGLint,
        }
        const PLANE_KEYS: [PlaneKey; 4] = [
            PlaneKey {
                fd: EGL_DMA_BUF_PLANE0_FD_EXT,
                offset: EGL_DMA_BUF_PLANE0_OFFSET_EXT,
                pitch: EGL_DMA_BUF_PLANE0_PITCH_EXT,
                mod_lo: EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT,
                mod_hi: EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT,
            },
            PlaneKey {
                fd: EGL_DMA_BUF_PLANE1_FD_EXT,
                offset: EGL_DMA_BUF_PLANE1_OFFSET_EXT,
                pitch: EGL_DMA_BUF_PLANE1_PITCH_EXT,
                mod_lo: EGL_DMA_BUF_PLANE1_MODIFIER_LO_EXT,
                mod_hi: EGL_DMA_BUF_PLANE1_MODIFIER_HI_EXT,
            },
            PlaneKey {
                fd: EGL_DMA_BUF_PLANE2_FD_EXT,
                offset: EGL_DMA_BUF_PLANE2_OFFSET_EXT,
                pitch: EGL_DMA_BUF_PLANE2_PITCH_EXT,
                mod_lo: EGL_DMA_BUF_PLANE2_MODIFIER_LO_EXT,
                mod_hi: EGL_DMA_BUF_PLANE2_MODIFIER_HI_EXT,
            },
            PlaneKey {
                fd: EGL_DMA_BUF_PLANE3_FD_EXT,
                offset: EGL_DMA_BUF_PLANE3_OFFSET_EXT,
                pitch: EGL_DMA_BUF_PLANE3_PITCH_EXT,
                mod_lo: EGL_DMA_BUF_PLANE3_MODIFIER_LO_EXT,
                mod_hi: EGL_DMA_BUF_PLANE3_MODIFIER_HI_EXT,
            },
        ];

        let mut attribs = Vec::with_capacity(19);
        attribs.extend_from_slice(&[EGL_WIDTH, buf.width]);
        attribs.extend_from_slice(&[EGL_HEIGHT, buf.height]);
        attribs.extend_from_slice(&[EGL_LINUX_DRM_FOURCC_EXT, buf.format.drm as _]);
        attribs.extend_from_slice(&[EGL_IMAGE_PRESERVED_KHR, EGL_TRUE as _]);
        for (key, plane) in PLANE_KEYS.iter().zip(buf.planes.iter()) {
            attribs.extend_from_slice(&[key.fd, plane.fd.raw()]);
            attribs.extend_from_slice(&[key.pitch, plane.stride as _]);
            attribs.extend_from_slice(&[key.offset, plane.offset as _]);
            if buf.modifier != INVALID_MODIFIER {
                attribs.extend_from_slice(&[key.mod_lo, buf.modifier as i32]);
                attribs.extend_from_slice(&[key.mod_hi, (buf.modifier >> 32) as i32]);
            }
        }
        attribs.push(EGL_NONE);
        let img = unsafe {
            self.procs.eglCreateImageKHR(
                self.dpy,
                EGLContext::none(),
                EGL_LINUX_DMA_BUF_EXT as _,
                EGLClientBuffer::none(),
                attribs.as_ptr(),
            )
        };
        if img.is_none() {
            return Err(RenderError::CreateImage);
        }
        Ok(Rc::new(EglImage {
            dpy: self.clone(),
            img,
            external_only: format.external_only,
            dmabuf: buf.clone(),
        }))
    }
}

unsafe fn query_formats(
    procs: &ExtProc,
    dpy: EGLDisplay,
) -> Result<BHashMap<u32, EglFormat>, RenderError> {
    let mut vec = vec![];
    let mut num = 0;
    let res = unsafe { procs.eglQueryDmaBufFormatsEXT(dpy, num, ptr::null_mut(), &mut num) };
    if res != EGL_TRUE {
        return Err(RenderError::QueryDmaBufFormats);
    }
    vec.reserve_exact(num as usize);
    let res = unsafe { procs.eglQueryDmaBufFormatsEXT(dpy, num, vec.as_mut_ptr(), &mut num) };
    if res != EGL_TRUE {
        return Err(RenderError::QueryDmaBufFormats);
    }
    unsafe {
        vec.set_len(num as usize);
    }
    let mut res = BHashMap::default();
    let formats = formats();
    for fmt in vec {
        if let Some(format) = formats.get(&(fmt as u32)) {
            let (modifiers, external_only) = unsafe { query_modifiers(procs, dpy, fmt, format)? };
            res.insert(
                format.drm,
                EglFormat {
                    format,
                    implicit_external_only: external_only,
                    modifiers,
                },
            );
        }
    }
    Ok(res)
}

unsafe fn query_modifiers(
    procs: &ExtProc,
    dpy: EGLDisplay,
    gl_format: EGLint,
    format: &'static Format,
) -> Result<(IndexMap<Modifier, EglModifier>, bool), RenderError> {
    let mut mods = vec![];
    let mut ext_only = vec![];
    let mut num = 0;
    let res = unsafe {
        procs.eglQueryDmaBufModifiersEXT(
            dpy,
            gl_format,
            num,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut num,
        )
    };
    if res != EGL_TRUE {
        return Err(RenderError::QueryDmaBufModifiers);
    }
    mods.reserve_exact(num as usize);
    ext_only.reserve_exact(num as usize);
    let res = unsafe {
        procs.eglQueryDmaBufModifiersEXT(
            dpy,
            gl_format,
            num,
            mods.as_mut_ptr(),
            ext_only.as_mut_ptr(),
            &mut num,
        )
    };
    if res != EGL_TRUE {
        return Err(RenderError::QueryDmaBufModifiers);
    }
    unsafe {
        mods.set_len(num as usize);
        ext_only.set_len(num as usize);
    }
    let mut res = IndexMap::new();
    for (modifier, ext_only) in mods.iter().copied().zip(ext_only.iter().copied()) {
        res.insert(
            modifier as _,
            EglModifier {
                modifier: modifier as _,
                external_only: ext_only == EGL_TRUE,
            },
        );
    }
    let mut external_only = format.external_only_guess;
    if res.len() > 0 {
        external_only = res.values().any(|f| f.external_only);
    }
    res.insert(
        INVALID_MODIFIER,
        EglModifier {
            modifier: INVALID_MODIFIER,
            external_only,
        },
    );
    Ok((res, external_only))
}

unsafe fn query_devices(procs: &ExtProc) -> Result<Vec<EGLDeviceEXT>, RenderError> {
    let mut vec = vec![];
    let mut num = 0;
    let res = unsafe { procs.eglQueryDevicesEXT(num, ptr::null_mut(), &mut num) };
    if res != EGL_TRUE {
        return Err(RenderError::QueryDevices);
    }
    if num > 0 {
        vec.reserve_exact(num as usize);
        let res = unsafe { procs.eglQueryDevicesEXT(num, vec.as_mut_ptr(), &mut num) };
        if res != EGL_TRUE {
            return Err(RenderError::QueryDevices);
        }
        unsafe {
            vec.set_len(num as usize);
        }
    }
    Ok(vec)
}

thread_local! {
    static DISPLAY_CACHE: CopyHashMap<EGLDisplay, Weak<EglDisplayHolder>> = CopyHashMap::new();
}

unsafe fn get_or_create_display(
    egl: &'static Egl,
    procs: &ExtProc,
    drm: &Drm,
    software: bool,
) -> Result<Rc<EglDisplayHolder>, RenderError> {
    unsafe {
        let devices = query_devices(procs)?;
        let device = match software {
            false => find_device(procs, &devices, drm)?,
            true => find_software_device(procs, &devices)?,
        };
        let dpy =
            procs.eglGetPlatformDisplayEXT(EGL_PLATFORM_DEVICE_EXT as _, device.0, ptr::null());
        if dpy.is_none() {
            return Err(RenderError::GetDisplay);
        }
        DISPLAY_CACHE.with(|cache| {
            if let Some(holder) = cache.get(&dpy)
                && let Some(holder) = holder.upgrade()
            {
                return Ok(holder);
            }
            let mut major = 0;
            let mut minor = 0;
            if (egl.eglInitialize)(dpy, &mut major, &mut minor) != EGL_TRUE {
                return Err(RenderError::Initialize);
            }
            let holder = Rc::new(EglDisplayHolder { egl, dpy });
            cache.set(dpy, Rc::downgrade(&holder));
            Ok(holder)
        })
    }
}

impl Drop for EglDisplayHolder {
    fn drop(&mut self) {
        DISPLAY_CACHE.with(|cache| {
            cache.remove(&self.dpy);
        });
        unsafe {
            if (self.egl.eglTerminate)(self.dpy) != EGL_TRUE {
                log::warn!("`eglTerminate` failed");
            }
        }
    }
}

unsafe fn find_device(
    procs: &ExtProc,
    devices: &[EGLDeviceEXT],
    drm: &Drm,
) -> Result<EGLDeviceEXT, RenderError> {
    let dev = drm.dev();
    let files = [
        (EXT_DEVICE_DRM, EGL_DRM_DEVICE_FILE_EXT),
        (EXT_DEVICE_DRM_RENDER_NODE, EGL_DRM_RENDER_NODE_FILE_EXT),
    ];
    for &device in devices {
        let exts = get_device_ext(procs, device);
        if exts.contains(MESA_DEVICE_SOFTWARE) {
            continue;
        }
        for (ext, name) in files {
            if !exts.contains(ext) {
                continue;
            }
            let file = unsafe { procs.eglQueryDeviceStringEXT(device, name) };
            if file.is_null() {
                continue;
            }
            let file = unsafe { CStr::from_ptr(file) };
            if let Ok(stat) = uapi::stat(file)
                && stat.st_rdev == dev
            {
                return Ok(device);
            }
        }
    }
    Err(RenderError::NoDeviceFound(dev))
}

unsafe fn find_software_device(
    procs: &ExtProc,
    devices: &[EGLDeviceEXT],
) -> Result<EGLDeviceEXT, RenderError> {
    devices
        .iter()
        .find(|&&device| get_device_ext(procs, device).contains(MESA_DEVICE_SOFTWARE))
        .copied()
        .ok_or(RenderError::NoSoftwareRenderer)
}
