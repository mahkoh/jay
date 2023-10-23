use {
    crate::{
        cursor::Cursor,
        fixed::Fixed,
        format::{Format, ARGB8888, XRGB8888},
        gfx_api::{GfxFramebuffer, GfxTexture},
        gfx_apis::gl::{
            gl::{
                frame_buffer::GlFrameBuffer,
                sys::{
                    glBindFramebuffer, glClear, glClearColor, glViewport, GL_COLOR_BUFFER_BIT,
                    GL_FRAMEBUFFER,
                },
            },
            renderer::context::GlRenderContext,
            run_ops,
            sys::{glBlendFunc, glFlush, glReadnPixels, GL_ONE, GL_ONE_MINUS_SRC_ALPHA},
        },
        rect::Rect,
        renderer::{renderer_base::RendererBase, RenderResult, Renderer},
        scale::Scale,
        state::State,
        tree::Node,
    },
    std::{
        any::Any,
        cell::Cell,
        fmt::{Debug, Formatter},
        rc::Rc,
    },
};

pub struct Framebuffer {
    pub(in crate::gfx_apis::gl) ctx: Rc<GlRenderContext>,
    pub(in crate::gfx_apis::gl) gl: GlFrameBuffer,
}

impl Debug for Framebuffer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Framebuffer").finish_non_exhaustive()
    }
}

impl Framebuffer {
    pub fn clear(&self) {
        self.clear_with(0.0, 0.0, 0.0, 0.0);
    }

    pub fn clear_with(&self, r: f32, g: f32, b: f32, a: f32) {
        let _ = self.ctx.ctx.with_current(|| {
            unsafe {
                glBindFramebuffer(GL_FRAMEBUFFER, self.gl.fbo);
                glViewport(0, 0, self.gl.width, self.gl.height);
                glClearColor(r, g, b, a);
                glClear(GL_COLOR_BUFFER_BIT);
            }
            Ok(())
        });
    }

    pub fn copy_texture(
        &self,
        state: &State,
        texture: &Rc<dyn GfxTexture>,
        x: i32,
        y: i32,
        alpha: bool,
    ) {
        let mut ops = self.ctx.gfx_ops.borrow_mut();
        ops.clear();
        let scale = Scale::from_int(1);
        let extents = Rect::new_sized(0, 0, self.gl.width, self.gl.height).unwrap();
        let mut renderer = Renderer {
            base: RendererBase {
                ops: &mut ops,
                scaled: false,
                scale,
                scalef: 1.0,
            },
            state,
            result: None,
            logical_extents: extents,
            physical_extents: extents,
        };
        let format = match alpha {
            true => ARGB8888,
            false => XRGB8888,
        };
        renderer
            .base
            .render_texture(texture, x, y, format, None, None, scale, i32::MAX, i32::MAX);
        let _ = self.ctx.ctx.with_current(|| {
            unsafe {
                glBindFramebuffer(GL_FRAMEBUFFER, self.gl.fbo);
                glViewport(0, 0, self.gl.width, self.gl.height);
                if alpha {
                    glClearColor(0.0, 0.0, 0.0, 0.0);
                    glClear(GL_COLOR_BUFFER_BIT);
                }
                glBlendFunc(GL_ONE, GL_ONE_MINUS_SRC_ALPHA);
            }
            run_ops(self, &ops);
            unsafe {
                glFlush();
            }
            Ok(())
        });
    }

    pub fn copy_to_shm(
        &self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        format: &Format,
        shm: &[Cell<u8>],
    ) {
        let y = self.gl.height - y - height;
        let _ = self.ctx.ctx.with_current(|| {
            unsafe {
                glBindFramebuffer(GL_FRAMEBUFFER, self.gl.fbo);
                glViewport(0, 0, self.gl.width, self.gl.height);
                glReadnPixels(
                    x,
                    y,
                    width,
                    height,
                    format.gl_format as _,
                    format.gl_type as _,
                    shm.len() as _,
                    shm.as_ptr() as _,
                );
            }
            Ok(())
        });
    }

    pub fn render_custom(&self, scale: Scale, f: &mut dyn FnMut(&mut RendererBase)) {
        let mut ops = self.ctx.gfx_ops.borrow_mut();
        ops.clear();
        let mut renderer = RendererBase {
            ops: &mut ops,
            scaled: scale != 1,
            scale,
            scalef: scale.to_f64(),
        };
        f(&mut renderer);
        let _ = self.ctx.ctx.with_current(|| {
            unsafe {
                glBindFramebuffer(GL_FRAMEBUFFER, self.gl.fbo);
                glViewport(0, 0, self.gl.width, self.gl.height);
                glBlendFunc(GL_ONE, GL_ONE_MINUS_SRC_ALPHA);
            }
            run_ops(self, &ops);
            unsafe {
                glFlush();
            }
            Ok(())
        });
    }

    pub fn render(
        &self,
        node: &dyn Node,
        state: &State,
        cursor_rect: Option<Rect>,
        result: Option<&mut RenderResult>,
        scale: Scale,
        render_hardware_cursor: bool,
    ) {
        let mut ops = self.ctx.gfx_ops.borrow_mut();
        ops.clear();
        let mut renderer = Renderer {
            base: RendererBase {
                ops: &mut ops,
                scaled: scale != 1,
                scale,
                scalef: scale.to_f64(),
            },
            state,
            result,
            logical_extents: node.node_absolute_position().at_point(0, 0),
            physical_extents: Rect::new(0, 0, self.gl.width, self.gl.height).unwrap(),
        };
        node.node_render(&mut renderer, 0, 0, i32::MAX, i32::MAX);
        if let Some(rect) = cursor_rect {
            let seats = state.globals.lock_seats();
            for seat in seats.values() {
                if let Some(cursor) = seat.get_cursor() {
                    let (mut x, mut y) = seat.get_position();
                    if let Some(dnd_icon) = seat.dnd_icon() {
                        let extents = dnd_icon.extents.get().move_(
                            x.round_down() + dnd_icon.buf_x.get(),
                            y.round_down() + dnd_icon.buf_y.get(),
                        );
                        if extents.intersects(&rect) {
                            let (x, y) = rect.translate(extents.x1(), extents.y1());
                            renderer.render_surface(&dnd_icon, x, y, i32::MAX, i32::MAX);
                        }
                    }
                    if render_hardware_cursor || !seat.hardware_cursor() {
                        cursor.tick();
                        x -= Fixed::from_int(rect.x1());
                        y -= Fixed::from_int(rect.y1());
                        cursor.render(&mut renderer, x, y);
                    }
                }
            }
        }
        let _ = self.ctx.ctx.with_current(|| {
            let c = state.theme.colors.background.get();
            unsafe {
                glBindFramebuffer(GL_FRAMEBUFFER, self.gl.fbo);
                glViewport(0, 0, self.gl.width, self.gl.height);
                glClearColor(c.r, c.g, c.b, 1.0);
                glClear(GL_COLOR_BUFFER_BIT);
                glBlendFunc(GL_ONE, GL_ONE_MINUS_SRC_ALPHA);
            }
            run_ops(self, &ops);
            unsafe {
                glFlush();
            }
            Ok(())
        });
    }

    pub fn render_hardware_cursor(&self, cursor: &dyn Cursor, state: &State, scale: Scale) {
        let mut ops = self.ctx.gfx_ops.borrow_mut();
        ops.clear();
        let mut renderer = Renderer {
            base: RendererBase {
                ops: &mut ops,
                scaled: scale != 1,
                scale,
                scalef: scale.to_f64(),
            },
            state,
            result: None,
            logical_extents: Rect::new_empty(0, 0),
            physical_extents: Rect::new(0, 0, self.gl.width, self.gl.height).unwrap(),
        };
        cursor.render_hardware_cursor(&mut renderer);
        let _ = self.ctx.ctx.with_current(|| {
            unsafe {
                glBindFramebuffer(GL_FRAMEBUFFER, self.gl.fbo);
                glViewport(0, 0, self.gl.width, self.gl.height);
                glClearColor(0.0, 0.0, 0.0, 0.0);
                glClear(GL_COLOR_BUFFER_BIT);
                glBlendFunc(GL_ONE, GL_ONE_MINUS_SRC_ALPHA);
            }
            run_ops(self, &ops);
            unsafe {
                glFlush();
            }
            Ok(())
        });
    }
}

impl GfxFramebuffer for Framebuffer {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clear(&self) {
        self.clear()
    }

    fn clear_with(&self, r: f32, g: f32, b: f32, a: f32) {
        self.clear_with(r, g, b, a)
    }

    fn copy_texture(
        &self,
        state: &State,
        texture: &Rc<dyn GfxTexture>,
        x: i32,
        y: i32,
        alpha: bool,
    ) {
        self.copy_texture(state, texture, x, y, alpha)
    }

    fn copy_to_shm(
        &self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        format: &Format,
        shm: &[Cell<u8>],
    ) {
        self.copy_to_shm(x, y, width, height, format, shm)
    }

    fn render_custom(&self, scale: Scale, f: &mut dyn FnMut(&mut RendererBase)) {
        self.render_custom(scale, f)
    }

    fn render(
        &self,
        node: &dyn Node,
        state: &State,
        cursor_rect: Option<Rect>,
        result: Option<&mut RenderResult>,
        scale: Scale,
        render_hardware_cursor: bool,
    ) {
        self.render(
            node,
            state,
            cursor_rect,
            result,
            scale,
            render_hardware_cursor,
        )
    }

    fn render_hardware_cursor(&self, cursor: &dyn Cursor, state: &State, scale: Scale) {
        self.render_hardware_cursor(cursor, state, scale)
    }
}
