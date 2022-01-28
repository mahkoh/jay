use crate::rect::Rect;
use crate::render::egl::context::EglContext;
use crate::render::gl::render_buffer::GlRenderBuffer;
use crate::render::gl::sys::{glDeleteFramebuffers, GLuint};
use crate::render::gl::texture::GlTexture;
use crate::render::sys::{glDisable, glEnable, glScissor, GL_SCISSOR_TEST};
use crate::utils::ptr_ext::PtrExt;
use std::ptr;
use std::rc::Rc;

pub struct GlFrameBuffer {
    pub _rb: Option<Rc<GlRenderBuffer>>,
    pub _tex: Option<Rc<GlTexture>>,
    pub ctx: Rc<EglContext>,
    pub width: i32,
    pub height: i32,
    pub fbo: GLuint,
}

impl Drop for GlFrameBuffer {
    fn drop(&mut self) {
        let _ = self.ctx.with_current(|| {
            unsafe {
                glDeleteFramebuffers(1, &self.fbo);
            }
            Ok(())
        });
    }
}

pub unsafe fn with_scissor<T, F: FnOnce() -> T>(scissor: &Rect, f: F) -> T {
    #[thread_local]
    static mut SCISSOR: *const Rect = ptr::null();

    let prev = SCISSOR;
    if prev.is_null() {
        glEnable(GL_SCISSOR_TEST);
    }
    glScissor(
        scissor.x1(),
        scissor.y1(),
        scissor.width(),
        scissor.height(),
    );
    SCISSOR = scissor;
    let res = f();
    if prev.is_null() {
        glDisable(GL_SCISSOR_TEST);
    } else {
        let prev = prev.deref();
        glScissor(prev.x1(), prev.y1(), prev.width(), prev.height());
    }
    SCISSOR = prev;
    res
}
