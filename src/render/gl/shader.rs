use {
    crate::render::{
        egl::context::EglContext,
        gl::sys::{
            glCompileShader, glCreateShader, glDeleteShader, glGetShaderiv, glShaderSource, GLenum,
            GLuint, GL_COMPILE_STATUS, GL_FALSE,
        },
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
    pub unsafe fn compile(
        ctx: &Rc<EglContext>,
        ty: GLenum,
        src: &str,
    ) -> Result<Self, RenderError> {
        let shader = glCreateShader(ty);
        let res = GlShader {
            ctx: ctx.clone(),
            shader,
        };
        let len = src.len() as _;
        glShaderSource(shader, 1, &(src.as_ptr() as _), &len);
        glCompileShader(shader);

        let mut ok = 0;
        glGetShaderiv(shader, GL_COMPILE_STATUS, &mut ok);
        if ok == GL_FALSE as GLint {
            return Err(RenderError::ShaderCompileFailed);
        }
        Ok(res)
    }
}

impl Drop for GlShader {
    fn drop(&mut self) {
        let _ = self.ctx.with_current(|| unsafe {
            glDeleteShader(self.shader);
            Ok(())
        });
    }
}
