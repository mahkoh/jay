use {
    crate::{
        rect::Rect,
        render::{
            gl::{
                frame_buffer::GlFrameBuffer,
                sys::{
                    glBindFramebuffer, glClear, glClearColor, glViewport, GL_COLOR_BUFFER_BIT,
                    GL_FRAMEBUFFER,
                },
            },
            renderer::{context::RenderContext, renderer::Renderer},
            sys::{glBlendFunc, glFlush, GL_ONE, GL_ONE_MINUS_SRC_ALPHA},
            RenderResult,
        },
        state::State,
        tree::Node,
    },
    std::{
        fmt::{Debug, Formatter},
        rc::Rc,
    },
};

pub struct Framebuffer {
    pub(super) ctx: Rc<RenderContext>,
    pub(super) gl: GlFrameBuffer,
}

impl Debug for Framebuffer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Framebuffer").finish_non_exhaustive()
    }
}

impl Framebuffer {
    pub fn clear(&self) {
        let _ = self.ctx.ctx.with_current(|| {
            unsafe {
                glBindFramebuffer(GL_FRAMEBUFFER, self.gl.fbo);
                glViewport(0, 0, self.gl.width, self.gl.height);
                glClearColor(0.0, 0.0, 0.0, 0.0);
                glClear(GL_COLOR_BUFFER_BIT);
            }
            Ok(())
        });
    }

    pub fn render(
        &self,
        node: &dyn Node,
        state: &State,
        cursor_rect: Option<Rect>,
        on_output: bool,
        result: &mut RenderResult,
    ) {
        let _ = self.ctx.ctx.with_current(|| {
            let c = state.theme.background_color.get();
            unsafe {
                glBindFramebuffer(GL_FRAMEBUFFER, self.gl.fbo);
                glViewport(0, 0, self.gl.width, self.gl.height);
                glClearColor(c.r, c.g, c.b, 1.0);
                glClear(GL_COLOR_BUFFER_BIT);
                glBlendFunc(GL_ONE, GL_ONE_MINUS_SRC_ALPHA);
            }
            let mut renderer = Renderer {
                ctx: &self.ctx,
                fb: &self.gl,
                state,
                on_output,
                result,
            };
            node.node_render(&mut renderer, 0, 0);
            if let Some(rect) = cursor_rect {
                let seats = state.globals.lock_seats();
                for seat in seats.values() {
                    if let Some(cursor) = seat.get_cursor() {
                        cursor.tick();
                        let extents = cursor.extents();
                        if let Some(dnd_icon) = seat.dnd_icon() {
                            let (x_hot, y_hot) = cursor.get_hotspot();
                            let extents = dnd_icon.extents.get().move_(
                                extents.x1() + x_hot + dnd_icon.buf_x.get(),
                                extents.y1() + y_hot + dnd_icon.buf_y.get(),
                            );
                            if extents.intersects(&rect) {
                                let (x, y) = rect.translate(extents.x1(), extents.y1());
                                renderer.render_surface(&dnd_icon, x, y);
                            }
                        }
                        if extents.intersects(&rect) {
                            let (x, y) = rect.translate(extents.x1(), extents.y1());
                            cursor.render(&mut renderer, x, y);
                        }
                    }
                }
            }
            unsafe {
                glFlush();
            }
            Ok(())
        });
    }
}
