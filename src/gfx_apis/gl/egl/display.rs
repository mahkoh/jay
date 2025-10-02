use {
    crate::{
        format::{Format, formats},
        gfx_api::{GfxFormat, GfxWriteModifier},
        gfx_apis::gl::{
            RenderError,
            egl::{
                EXTS, PROCS,
                context::EglContext,
                image::EglImage,
                sys::{
                    EGL_CONTEXT_CLIENT_VERSION, EGL_DMA_BUF_PLANE0_FD_EXT,
                    EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT, EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT,
                    EGL_DMA_BUF_PLANE0_OFFSET_EXT, EGL_DMA_BUF_PLANE0_PITCH_EXT,
                    EGL_DMA_BUF_PLANE1_FD_EXT, EGL_DMA_BUF_PLANE1_MODIFIER_HI_EXT,
                    EGL_DMA_BUF_PLANE1_MODIFIER_LO_EXT, EGL_DMA_BUF_PLANE1_OFFSET_EXT,
                    EGL_DMA_BUF_PLANE1_PITCH_EXT, EGL_DMA_BUF_PLANE2_FD_EXT,
                    EGL_DMA_BUF_PLANE2_MODIFIER_HI_EXT, EGL_DMA_BUF_PLANE2_MODIFIER_LO_EXT,
                    EGL_DMA_BUF_PLANE2_OFFSET_EXT, EGL_DMA_BUF_PLANE2_PITCH_EXT,
                    EGL_DMA_BUF_PLANE3_FD_EXT, EGL_DMA_BUF_PLANE3_MODIFIER_HI_EXT,
                    EGL_DMA_BUF_PLANE3_MODIFIER_LO_EXT, EGL_DMA_BUF_PLANE3_OFFSET_EXT,
                    EGL_DMA_BUF_PLANE3_PITCH_EXT, EGL_HEIGHT, EGL_IMAGE_PRESERVED_KHR,
                    EGL_LINUX_DMA_BUF_EXT, EGL_LINUX_DRM_FOURCC_EXT, EGL_NONE, EGL_TRUE, EGL_WIDTH,
                    EGLClientBuffer, EGLConfig, EGLContext, EGLDisplay, EGLint,
                },
            },
            ext::{
                ANDROID_NATIVE_FENCE_SYNC, DisplayExt, EXT_CREATE_CONTEXT_ROBUSTNESS,
                EXT_DEVICE_QUERY, EXT_IMAGE_DMA_BUF_IMPORT_MODIFIERS, GL_OES_EGL_IMAGE,
                GL_OES_EGL_IMAGE_EXTERNAL, GlExt, KHR_FENCE_SYNC, KHR_IMAGE_BASE,
                KHR_NO_CONFIG_CONTEXT, KHR_SURFACELESS_CONTEXT, KHR_WAIT_SYNC,
                MESA_CONFIGLESS_CONTEXT, MESA_DEVICE_SOFTWARE, get_device_ext, get_display_ext,
                get_gl_ext,
            },
            proc::ExtProc,
            sys::{
                EGL, EGL_CONTEXT_OPENGL_RESET_NOTIFICATION_STRATEGY_EXT,
                EGL_LOSE_CONTEXT_ON_RESET_EXT, EGL_PLATFORM_GBM_KHR, Egl, GLESV2, GlesV2,
            },
        },
        video::{INVALID_MODIFIER, Modifier, dmabuf::DmaBuf, drm::Drm, gbm::GbmDevice},
    },
    ahash::AHashMap,
    indexmap::{IndexMap, IndexSet},
    std::{ptr, rc::Rc},
};

#[derive(Debug)]
pub struct EglFormat {
    pub format: &'static Format,
    pub implicit_external_only: bool,
    pub modifiers: IndexMap<u64, EglModifier>,
}

#[derive(Debug)]
pub struct EglModifier {
    pub modifier: u64,
    pub external_only: bool,
}

#[derive(Debug)]
pub struct EglDisplay {
    pub egl: &'static Egl,
    pub gles: &'static GlesV2,
    pub procs: &'static ExtProc,
    pub exts: DisplayExt,
    pub formats: AHashMap<u32, EglFormat>,
    pub gbm: Rc<GbmDevice>,
    pub dpy: EGLDisplay,
    pub explicit_sync: bool,
    pub fast_ram_access: bool,
}

impl EglDisplay {
    pub(in crate::gfx_apis::gl) fn create(
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
            let dpy = procs.eglGetPlatformDisplayEXT(
                EGL_PLATFORM_GBM_KHR as _,
                gbm.raw() as _,
                ptr::null(),
            );
            if dpy.is_none() {
                return Err(RenderError::GetDisplay);
            }
            let mut dpy = EglDisplay {
                egl,
                gles,
                procs,
                exts: DisplayExt::none(),
                formats: AHashMap::new(),
                gbm: Rc::new(gbm),
                dpy,
                explicit_sync: false,
                fast_ram_access: false,
            };
            let mut major = 0;
            let mut minor = 0;
            if (egl.eglInitialize)(dpy.dpy, &mut major, &mut minor) != EGL_TRUE {
                return Err(RenderError::Initialize);
            }
            if EXTS.contains(EXT_DEVICE_QUERY)
                && get_device_ext(procs, dpy.dpy)?.contains(MESA_DEVICE_SOFTWARE)
            {
                if !software {
                    return Err(RenderError::NoHardwareRenderer);
                }
                dpy.fast_ram_access = true;
            }
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
                log::error!("Driver does not support explicit sync. Rendering will block.")
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
            let mut formats = AHashMap::new();
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
        buf: &DmaBuf,
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

impl Drop for EglDisplay {
    fn drop(&mut self) {
        unsafe {
            if (self.egl.eglTerminate)(self.dpy) != EGL_TRUE {
                log::warn!("`eglTerminate` failed");
            }
        }
    }
}

unsafe fn query_formats(
    procs: &ExtProc,
    dpy: EGLDisplay,
) -> Result<AHashMap<u32, EglFormat>, RenderError> {
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
    let mut res = AHashMap::new();
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
