use {
    crate::{
        format::Format,
        gfx_apis::gl::{
            egl::{context::EglContext, image::EglImage},
            ext::GL_OES_EGL_IMAGE_EXTERNAL,
            gl::sys::{
                GLint, GLuint, GL_CLAMP_TO_EDGE, GL_TEXTURE_2D, GL_TEXTURE_WRAP_S,
                GL_TEXTURE_WRAP_T, GL_UNPACK_ROW_LENGTH_EXT,
            },
            sys::{GLeglImageOES, GLenum, GL_TEXTURE_EXTERNAL_OES},
            RenderError,
        },
    },
    std::{cell::Cell, rc::Rc},
};

pub struct GlTexture {
    pub(crate) ctx: Rc<EglContext>,
    pub img: Option<Rc<EglImage>>,
    pub tex: GLuint,
    pub width: i32,
    pub height: i32,
    pub stride: i32,
    pub external_only: bool,
    pub format: &'static Format,
    pub contents_valid: Cell<bool>,
}

pub fn image_target(external_only: bool) -> GLenum {
    match external_only {
        true => GL_TEXTURE_EXTERNAL_OES,
        false => GL_TEXTURE_2D,
    }
}

impl GlTexture {
    pub(in crate::gfx_apis::gl) fn import_img(
        ctx: &Rc<EglContext>,
        img: &Rc<EglImage>,
    ) -> Result<GlTexture, RenderError> {
        if !ctx.ext.contains(GL_OES_EGL_IMAGE_EXTERNAL) {
            return Err(RenderError::ExternalUnsupported);
        }
        let gles = ctx.dpy.gles;
        let target = image_target(img.external_only);
        let tex = ctx.with_current(|| unsafe {
            let mut tex = 0;
            (gles.glGenTextures)(1, &mut tex);
            (gles.glBindTexture)(target, tex);
            (gles.glTexParameteri)(target, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
            (gles.glTexParameteri)(target, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
            ctx.dpy
                .procs
                .glEGLImageTargetTexture2DOES(target, GLeglImageOES(img.img.0));
            (gles.glBindTexture)(target, 0);
            Ok(tex)
        })?;
        Ok(GlTexture {
            ctx: ctx.clone(),
            img: Some(img.clone()),
            tex,
            width: img.dmabuf.width,
            height: img.dmabuf.height,
            stride: 0,
            external_only: img.external_only,
            format: img.dmabuf.format,
            contents_valid: Cell::new(true),
        })
    }

    pub(in crate::gfx_apis::gl) fn import_shm(
        ctx: &Rc<EglContext>,
        data: &[Cell<u8>],
        format: &'static Format,
        width: i32,
        height: i32,
        stride: i32,
    ) -> Result<GlTexture, RenderError> {
        let Some(shm_info) = &format.shm_info else {
            return Err(RenderError::UnsupportedShmFormat(format.name));
        };
        if (stride * height) as usize > data.len() {
            return Err(RenderError::SmallImageBuffer);
        }
        let gles = ctx.dpy.gles;
        let tex = ctx.with_current(|| unsafe {
            let mut tex = 0;
            (gles.glGenTextures)(1, &mut tex);
            (gles.glBindTexture)(GL_TEXTURE_2D, tex);
            (gles.glTexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
            (gles.glTexParameteri)(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
            (gles.glPixelStorei)(GL_UNPACK_ROW_LENGTH_EXT, stride / shm_info.bpp as GLint);
            (gles.glTexImage2D)(
                GL_TEXTURE_2D,
                0,
                shm_info.gl_format,
                width,
                height,
                0,
                shm_info.gl_format as _,
                shm_info.gl_type as _,
                data.as_ptr() as _,
            );
            (gles.glPixelStorei)(GL_UNPACK_ROW_LENGTH_EXT, 0);
            (gles.glBindTexture)(GL_TEXTURE_2D, 0);
            Ok(tex)
        })?;
        Ok(GlTexture {
            ctx: ctx.clone(),
            img: None,
            tex,
            width,
            height,
            stride,
            external_only: false,
            format,
            contents_valid: Cell::new(true),
        })
    }
}

impl Drop for GlTexture {
    fn drop(&mut self) {
        let _ = self.ctx.with_current(|| unsafe {
            (self.ctx.dpy.gles.glDeleteTextures)(1, &self.tex);
            Ok(())
        });
    }
}
