macro_rules! egl_transparent {
    ($name:ident) => {
        #[derive(Copy, Clone, Debug, Eq, PartialEq)]
        #[repr(transparent)]
        pub struct $name(pub *mut u8);

        impl $name {
            #[allow(clippy::allow_attributes, dead_code)]
            pub const fn none() -> Self {
                Self(std::ptr::null_mut())
            }

            #[allow(clippy::allow_attributes, dead_code)]
            pub fn is_none(self) -> bool {
                self.0.is_null()
            }
        }
    };
}

macro_rules! dynload {
    (
        $item:ident: $container:ident from $name:literal {
            $(
                $fun:ident: $ty:ty,
            )*
        }
    ) => {
        #[expect(non_snake_case)]
        #[derive(Debug)]
        pub struct $container {
            _lib: libloading::Library,
            $(
                pub $fun: $ty,
            )*
        }

        pub static $item: once_cell::sync::Lazy<Option<$container>> = once_cell::sync::Lazy::new(|| unsafe {
            use crate::utils::errorfmt::ErrorFmt;
            let lib = match libloading::Library::new($name) {
                Ok(l) => l,
                Err(e) => {
                    log::error!("Could not load lib{}: {}", $name, ErrorFmt(e));
                    return None;
                }
            };
            $(
                #[expect(non_snake_case)]
                let $fun: $ty =
                    match lib.get(stringify!($fun).as_bytes()) {
                        Ok(s) => *s,
                        Err(e) => {
                            log::error!("Could not load {} from {}: {}", stringify!($fun), $name, ErrorFmt(e));
                            return None;
                        }
                    };
            )*
            Some($container {
                _lib: lib,
                $(
                    $fun,
                )*
            })
        });
    };
}

use {
    crate::{
        cmm::cmm_transfer_function::TransferFunction,
        gfx_api::{
            AcquireSync, CopyTexture, FillRect, GfxApiOpt, GfxContext, GfxError, GfxTexture,
            ReleaseSync, SyncFile,
        },
        gfx_apis::gl::{
            egl::image::EglImage,
            gl::texture::image_target,
            renderer::{
                context::{GlRenderContext, TexCopyType, TexSourceType},
                framebuffer::Framebuffer,
                texture::Texture,
            },
            sys::{
                GL_BLEND, GL_FALSE, GL_FLOAT, GL_LINEAR, GL_TEXTURE_MIN_FILTER, GL_TEXTURE0,
                GL_TRIANGLE_STRIP, GL_TRIANGLES,
            },
        },
        theme::Color,
        utils::{errorfmt::ErrorFmt, rc_eq::rc_eq, vecstorage::VecStorage},
        video::{
            dmabuf::DMA_BUF_SYNC_READ,
            drm::{Drm, DrmError},
            gbm::GbmError,
        },
    },
    isnt::std_1::vec::IsntVecExt,
    once_cell::sync::Lazy,
    std::{cell::RefCell, error::Error, rc::Rc, sync::Arc},
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

static INIT: Lazy<Result<(), Arc<RenderError>>> = Lazy::new(|| egl::init().map_err(Arc::new));

pub(super) fn create_gfx_context(drm: &Drm) -> Result<Rc<dyn GfxContext>, GfxError> {
    if let Err(e) = &*INIT {
        return Err(GfxError(Box::new(e.clone())));
    }
    GlRenderContext::from_drm_device(drm)
        .map(|v| Rc::new(v) as Rc<dyn GfxContext>)
        .map_err(|e| e.into())
}

#[derive(Debug, Error)]
enum RenderError {
    #[error("Could not load libEGL")]
    LoadEgl,
    #[error("Could not load libGLESv2")]
    LoadGlesV2,
    #[error("Could not load extension functions from libEGL")]
    LoadEglProcs,
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
    #[error("`eglQueryDmaBufModifiersEXT` failed")]
    QueryDmaBufModifiers,
    #[error(transparent)]
    DrmError(#[from] DrmError),
    #[error("The GLES driver does not support the XRGB8888 format")]
    XRGB888,
    #[error("The DRM device does not have a render node")]
    NoRenderNode,
    #[error("The requested format is not supported")]
    UnsupportedFormat,
    #[error("The requested modifier is not supported")]
    UnsupportedModifier,
    #[error("Image is external only and cannot be rendered to")]
    ExternalOnly,
    #[error("OpenGL context does not support external textures")]
    ExternalUnsupported,
    #[error("OpenGL context does not support any formats")]
    NoSupportedFormats,
    #[error("Could not create EGLSyncKHR")]
    CreateEglSync,
    #[error("Could not destroy EGLSyncKHR")]
    DestroyEglSync,
    #[error("Could not export sync file")]
    ExportSyncFile,
    #[error("Could not insert wait for EGLSyncKHR")]
    WaitSync,
    #[error("Buffer format {0} is not supported for shm buffers in OpenGL context")]
    UnsupportedShmFormat(&'static str),
    #[error("Could not access the client memory")]
    AccessFailed(#[source] Box<dyn Error + Sync + Send>),
    #[error("OpenGL does not support blend buffers")]
    NoBlendBuffer,
}

#[derive(Default)]
struct GfxGlState {
    triangles: RefCell<Vec<[f32; 2]>>,
    fill_rect: VecStorage<FillRect>,
    copy_tex: VecStorage<&'static CopyTexture>,
}

fn run_ops(fb: &Framebuffer, ops: &[GfxApiOpt]) -> Option<SyncFile> {
    let mut state = fb.ctx.gl_state.borrow_mut();
    let state = &mut *state;
    let mut fill_rect = state.fill_rect.take();
    let fill_rect = &mut *fill_rect;
    let mut copy_tex = state.copy_tex.take();
    let copy_tex = &mut *copy_tex;
    let mut triangles = state.triangles.borrow_mut();
    let triangles = &mut *triangles;
    let mut i = 0;
    while i < ops.len() {
        macro_rules! has_ops {
            () => {
                fill_rect.is_not_empty() || copy_tex.is_not_empty()
            };
        }
        fill_rect.clear();
        copy_tex.clear();
        while i < ops.len() {
            match &ops[i] {
                GfxApiOpt::Sync => {
                    i += 1;
                    if has_ops!() {
                        break;
                    }
                }
                GfxApiOpt::FillRect(f) => {
                    fill_rect.push(FillRect {
                        rect: f.rect,
                        color: f.effective_color(),
                        alpha: None,
                    });
                    i += 1;
                }
                GfxApiOpt::CopyTexture(c) => {
                    copy_tex.push(c);
                    i += 1;
                }
            }
        }
        if fill_rect.is_not_empty() {
            fill_rect.sort_unstable_by_key(|f| f.color);
            let mut i = 0;
            while i < fill_rect.len() {
                triangles.clear();
                let mut color = None;
                while i < fill_rect.len() {
                    let fr = &fill_rect[i];
                    match color {
                        None => color = Some(fr.color),
                        Some(c) if c == fr.color => {}
                        _ => break,
                    }
                    let [top_right, top_left, bottom_right, bottom_left] = fr.rect.to_points();
                    triangles.extend_from_slice(&[
                        top_right,
                        top_left,
                        bottom_left,
                        top_right,
                        bottom_left,
                        bottom_right,
                    ]);
                    i += 1;
                }
                if let Some(color) = color {
                    fill_boxes3(&fb.ctx, triangles, &color);
                }
            }
        }
        for tex in &*copy_tex {
            render_texture(&fb.ctx, tex);
        }
    }
    if fb.ctx.ctx.dpy.explicit_sync {
        let file = match fb.ctx.ctx.export_sync_file() {
            Ok(f) => SyncFile(Rc::new(f)),
            Err(e) => {
                log::error!("Could not create sync file: {}", ErrorFmt(e));
                return None;
            }
        };
        let user = fb.ctx.buffer_resv_user;
        for op in ops {
            if let GfxApiOpt::CopyTexture(ct) = op {
                if ct.release_sync == ReleaseSync::Explicit {
                    if let Some(resv) = &ct.buffer_resv {
                        resv.set_sync_file(user, &file);
                    }
                }
            }
        }
        return Some(file);
    }
    None
}

fn fill_boxes3(ctx: &GlRenderContext, boxes: &[[f32; 2]], color: &Color) {
    let [r, g, b, a] = color.to_array(TransferFunction::Srgb);
    let gles = ctx.ctx.dpy.gles;
    unsafe {
        (gles.glUseProgram)(ctx.fill_prog.prog);
        (gles.glUniform4f)(ctx.fill_prog_color, r, g, b, a);
        (gles.glVertexAttribPointer)(
            ctx.fill_prog_pos as _,
            2,
            GL_FLOAT,
            GL_FALSE,
            0,
            boxes.as_ptr() as _,
        );
        (gles.glEnableVertexAttribArray)(ctx.fill_prog_pos as _);
        (gles.glDrawArrays)(GL_TRIANGLES, 0, boxes.len() as _);
        (gles.glDisableVertexAttribArray)(ctx.fill_prog_pos as _);
    }
}

fn render_texture(ctx: &GlRenderContext, tex: &CopyTexture) {
    let texture = tex.tex.as_gl();
    if !texture.gl.contents_valid.get() {
        log::error!("Ignoring texture with invalid contents");
        return;
    }
    assert!(rc_eq(&ctx.ctx, &texture.ctx.ctx));
    let gles = ctx.ctx.dpy.gles;
    unsafe {
        handle_explicit_sync(ctx, texture.gl.img.as_ref(), &tex.acquire_sync);

        (gles.glActiveTexture)(GL_TEXTURE0);

        let target = image_target(texture.gl.external_only);

        (gles.glBindTexture)(target, texture.gl.tex);
        (gles.glTexParameteri)(target, GL_TEXTURE_MIN_FILTER, GL_LINEAR);

        let progs = match texture.gl.external_only {
            true => match &ctx.tex_external {
                Some(p) => p,
                _ => {
                    log::error!(
                        "Trying to render an external-only texture but context does not support the required extension"
                    );
                    return;
                }
            },
            false => &ctx.tex_internal,
        };
        let copy_type = match tex.alpha.is_some() {
            true => TexCopyType::Multiply,
            false => TexCopyType::Identity,
        };
        let source_type = match texture.gl.format.has_alpha {
            true => TexSourceType::HasAlpha,
            false => TexSourceType::Opaque,
        };
        if (copy_type, source_type) == (TexCopyType::Identity, TexSourceType::Opaque) {
            (gles.glDisable)(GL_BLEND);
        } else {
            (gles.glEnable)(GL_BLEND);
        }
        let prog = &progs[copy_type][source_type];

        (gles.glUseProgram)(prog.prog.prog);

        (gles.glUniform1i)(prog.tex, 0);

        let texcoord = tex.source.to_points();
        let pos = tex.target.to_points();

        if let Some(alpha) = tex.alpha {
            (gles.glUniform1f)(prog.alpha, alpha);
        }

        (gles.glVertexAttribPointer)(
            prog.texcoord as _,
            2,
            GL_FLOAT,
            GL_FALSE,
            0,
            texcoord.as_ptr() as _,
        );
        (gles.glVertexAttribPointer)(prog.pos as _, 2, GL_FLOAT, GL_FALSE, 0, pos.as_ptr() as _);

        (gles.glEnableVertexAttribArray)(prog.texcoord as _);
        (gles.glEnableVertexAttribArray)(prog.pos as _);

        (gles.glDrawArrays)(GL_TRIANGLE_STRIP, 0, 4);

        (gles.glDisableVertexAttribArray)(prog.texcoord as _);
        (gles.glDisableVertexAttribArray)(prog.pos as _);

        (gles.glBindTexture)(target, 0);
    }
}

fn handle_explicit_sync(ctx: &GlRenderContext, img: Option<&Rc<EglImage>>, sync: &AcquireSync) {
    let sync_file = match sync {
        AcquireSync::None | AcquireSync::Implicit | AcquireSync::Unnecessary => return,
        AcquireSync::SyncFile { sync_file } => sync_file,
    };
    let sync_file = match uapi::fcntl_dupfd_cloexec(sync_file.raw(), 0) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Could not dup sync file: {}", ErrorFmt(e));
            return;
        }
    };
    if ctx.ctx.dpy.explicit_sync {
        let sync = match ctx.ctx.create_sync(Some(sync_file)) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Could import sync file: {}", ErrorFmt(e));
                return;
            }
        };
        sync.wait();
    } else {
        if let Some(img) = img {
            if let Err(e) = img.dmabuf.import_sync_file(DMA_BUF_SYNC_READ, &sync_file) {
                log::error!("Could not import sync file into dmabuf: {}", ErrorFmt(e));
            }
        }
    }
}

impl dyn GfxTexture {
    fn as_gl(&self) -> &Texture {
        self.as_any()
            .downcast_ref()
            .expect("Non-gl texture passed into gl")
    }
}

impl From<RenderError> for GfxError {
    fn from(value: RenderError) -> Self {
        Self(Box::new(value))
    }
}
