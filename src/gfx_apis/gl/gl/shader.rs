use {
    crate::gfx_apis::gl::{
        egl::context::EglContext,
        gl::sys::{GLenum, GLuint, GL_COMPILE_STATUS, GL_FALSE},
        sys::GLint,
        RenderError,
    },
    std::rc::Rc,
};

pub struct GlShader {
    pub ctx: Rc<EglContext>,
    pub shader: GLuint,
}

impl GlShader {
    pub(in crate::gfx_apis::gl) unsafe fn compile(
        ctx: &Rc<EglContext>,
        ty: GLenum,
        src: &str,
    ) -> Result<Self, RenderError> {
        let gles = ctx.dpy.gles;
        let shader = (gles.glCreateShader)(ty);
        let res = GlShader {
            ctx: ctx.clone(),
            shader,
        };
        let len = src.len() as _;
        (gles.glShaderSource)(shader, 1, &(src.as_ptr() as _), &len);
        (gles.glCompileShader)(shader);

        let mut ok = 0;
        (gles.glGetShaderiv)(shader, GL_COMPILE_STATUS, &mut ok);
        if ok == GL_FALSE as GLint {
            return Err(RenderError::ShaderCompileFailed);
        }
        Ok(res)
    }
}

impl Drop for GlShader {
    fn drop(&mut self) {
        let _ = self.ctx.with_current(|| unsafe {
            (self.ctx.dpy.gles.glDeleteShader)(self.shader);
            Ok(())
        });
    }
}
