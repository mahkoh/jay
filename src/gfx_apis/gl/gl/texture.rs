use {
    crate::{
        format::Format,
        gfx_apis::gl::{
            egl::{context::EglContext, image::EglImage, PROCS},
            ext::GlExt,
            gl::sys::{
                glBindTexture, glDeleteTextures, glGenTextures, glPixelStorei, glTexImage2D,
                glTexParameteri, GLint, GLuint, GL_CLAMP_TO_EDGE, GL_TEXTURE_2D, GL_TEXTURE_WRAP_S,
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
    pub external_only: bool,
}

pub fn image_target(external_only: bool) -> GLenum {
    match external_only {
        true => GL_TEXTURE_EXTERNAL_OES,
        false => GL_TEXTURE_2D,
    }
}

impl GlTexture {
    pub fn import_img(ctx: &Rc<EglContext>, img: &Rc<EglImage>) -> Result<GlTexture, RenderError> {
        if !ctx.ext.contains(GlExt::GL_OES_EGL_IMAGE_EXTERNAL) {
            return Err(RenderError::ExternalUnsupported);
        }
        let target = image_target(img.external_only);
        let tex = ctx.with_current(|| unsafe {
            let mut tex = 0;
            glGenTextures(1, &mut tex);
            glBindTexture(target, tex);
            glTexParameteri(target, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
            glTexParameteri(target, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
            PROCS.glEGLImageTargetTexture2DOES(target, GLeglImageOES(img.img.0));
            glBindTexture(target, 0);
            Ok(tex)
        })?;
        Ok(GlTexture {
            ctx: ctx.clone(),
            img: Some(img.clone()),
            tex,
            width: img.width,
            height: img.height,
            external_only: img.external_only,
        })
    }

    pub fn import_shm(
        ctx: &Rc<EglContext>,
        data: &[Cell<u8>],
        format: &'static Format,
        width: i32,
        height: i32,
        stride: i32,
    ) -> Result<GlTexture, RenderError> {
        if (stride * height) as usize > data.len() {
            return Err(RenderError::SmallImageBuffer);
        }
        let tex = ctx.with_current(|| unsafe {
            let mut tex = 0;
            glGenTextures(1, &mut tex);
            glBindTexture(GL_TEXTURE_2D, tex);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
            glPixelStorei(GL_UNPACK_ROW_LENGTH_EXT, stride / format.bpp as GLint);
            glTexImage2D(
                GL_TEXTURE_2D,
                0,
                format.gl_format,
                width,
                height,
                0,
                format.gl_format as _,
                format.gl_type as _,
                data.as_ptr() as _,
            );
            glPixelStorei(GL_UNPACK_ROW_LENGTH_EXT, 0);
            glBindTexture(GL_TEXTURE_2D, 0);
            Ok(tex)
        })?;
        Ok(GlTexture {
            ctx: ctx.clone(),
            img: None,
            tex,
            width,
            height,
            external_only: false,
        })
    }
}

impl Drop for GlTexture {
    fn drop(&mut self) {
        let _ = self.ctx.with_current(|| unsafe {
            glDeleteTextures(1, &self.tex);
            Ok(())
        });
    }
}
