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

macro_rules! dynload {
    (
        $item:ident: $container:ident from $name:literal {
            $(
                $fun:ident: $ty:ty,
            )*
        }
    ) => {
        #[allow(non_snake_case)]
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
                #[allow(non_snake_case)]
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
        gfx_api::{
            CopyTexture, FillRect, FramebufferRect, GfxApiOpt, GfxContext, GfxError, GfxTexture,
            SampleRect,
        },
        gfx_apis::gl::{
            gl::texture::image_target,
            renderer::{context::GlRenderContext, framebuffer::Framebuffer, texture::Texture},
            sys::{
                GL_BLEND, GL_FALSE, GL_FLOAT, GL_LINEAR, GL_TEXTURE0, GL_TEXTURE_MIN_FILTER,
                GL_TRIANGLES, GL_TRIANGLE_STRIP,
            },
        },
        theme::Color,
        utils::{rc_eq::rc_eq, vecstorage::VecStorage},
        video::{
            drm::{Drm, DrmError},
            gbm::GbmError,
        },
    },
    isnt::std_1::vec::IsntVecExt,
    once_cell::sync::Lazy,
    std::{cell::RefCell, rc::Rc, sync::Arc},
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
    #[error("Cannot convert a shm texture into a framebuffer")]
    ShmTextureToFb,
}

#[derive(Default)]
struct GfxGlState {
    triangles: RefCell<Vec<[f32; 2]>>,
    fill_rect: VecStorage<&'static FillRect>,
    copy_tex: VecStorage<&'static CopyTexture>,
}

fn run_ops(fb: &Framebuffer, ops: &[GfxApiOpt]) {
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
                    fill_rect.push(f);
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
                    let fr = fill_rect[i];
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
            render_texture(&fb.ctx, &tex.tex.as_gl(), &tex.target, &tex.source)
        }
    }
}

fn fill_boxes3(ctx: &GlRenderContext, boxes: &[[f32; 2]], color: &Color) {
    let gles = ctx.ctx.dpy.gles;
    unsafe {
        (gles.glUseProgram)(ctx.fill_prog.prog);
        (gles.glUniform4f)(ctx.fill_prog_color, color.r, color.g, color.b, color.a);
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

fn render_texture(
    ctx: &GlRenderContext,
    texture: &Texture,
    target_rect: &FramebufferRect,
    src: &SampleRect,
) {
    assert!(rc_eq(&ctx.ctx, &texture.ctx.ctx));
    let gles = ctx.ctx.dpy.gles;
    unsafe {
        (gles.glActiveTexture)(GL_TEXTURE0);

        let target = image_target(texture.gl.external_only);

        (gles.glBindTexture)(target, texture.gl.tex);
        (gles.glTexParameteri)(target, GL_TEXTURE_MIN_FILTER, GL_LINEAR);

        let progs = match texture.gl.external_only {
            true => match &ctx.tex_external {
                Some(p) => p,
                _ => {
                    log::error!("Trying to render an external-only texture but context does not support the required extension");
                    return;
                }
            },
            false => &ctx.tex_internal,
        };
        let prog = match texture.gl.format.has_alpha {
            true => {
                (gles.glEnable)(GL_BLEND);
                &progs.alpha
            }
            false => {
                (gles.glDisable)(GL_BLEND);
                &progs.solid
            }
        };

        (gles.glUseProgram)(prog.prog.prog);

        (gles.glUniform1i)(prog.tex, 0);

        let texcoord = src.to_points();
        let pos = target_rect.to_points();

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
