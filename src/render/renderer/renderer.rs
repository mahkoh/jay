use {
    crate::{
        format::{Format, ARGB8888},
        ifs::{
            wl_buffer::WlBuffer,
            wl_surface::{
                xdg_surface::XdgSurface, zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, WlSurface,
            },
        },
        rect::Rect,
        render::{
            gl::{
                frame_buffer::{with_scissor, GlFrameBuffer},
                sys::{
                    glActiveTexture, glBindTexture, glDisableVertexAttribArray, glDrawArrays,
                    glEnableVertexAttribArray, glTexParameteri, glUniform1i, glUniform4f,
                    glUseProgram, glVertexAttribPointer, GL_FALSE, GL_FLOAT, GL_LINEAR,
                    GL_TEXTURE0, GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_TRIANGLES,
                    GL_TRIANGLE_STRIP,
                },
            },
            renderer::context::RenderContext,
            sys::{glDisable, glEnable, GL_BLEND},
            Texture,
        },
        state::State,
        theme::Color,
        tree::{ContainerNode, DisplayNode, FloatNode, OutputNode, PlaceholderNode, WorkspaceNode},
        utils::rc_eq::rc_eq,
    },
    std::{ops::Deref, rc::Rc, slice},
};

pub struct Renderer<'a> {
    pub(super) ctx: &'a Rc<RenderContext>,
    pub(super) fb: &'a GlFrameBuffer,
    pub(super) state: &'a State,
}

impl Renderer<'_> {
    pub fn render_display(&mut self, display: &DisplayNode, x: i32, y: i32) {
        let ext = display.extents.get();
        let outputs = display.outputs.lock();
        for output in outputs.values() {
            let opos = output.global.pos.get();
            let (ox, oy) = ext.translate(opos.x1(), opos.y1());
            self.render_output(output, x + ox, y + oy);
        }
    }

    pub fn render_output(&mut self, output: &OutputNode, x: i32, y: i32) {
        if let Some(ws) = output.workspace.get() {
            if let Some(fs) = ws.fullscreen.get() {
                fs.as_node().node_render(self, x, y);
                return;
            }
        }
        let opos = output.global.pos.get();
        macro_rules! render_layer {
            ($layer:expr) => {
                for ls in $layer.iter() {
                    let pos = ls.position();
                    self.render_layer_surface(
                        ls.deref(),
                        x + pos.x1() - opos.x1(),
                        y + pos.y1() - opos.y1(),
                    );
                }
            };
        }
        render_layer!(output.layers[0]);
        render_layer!(output.layers[1]);
        let theme = &self.state.theme;
        let th = theme.title_height.get();
        {
            let c = Color::BLACK;
            self.fill_boxes2(
                slice::from_ref(&Rect::new_sized(0, 0, opos.width(), th).unwrap()),
                &c,
                x,
                y,
            );
            let rd = output.render_data.borrow_mut();
            if let Some(aw) = &rd.active_workspace {
                let c = theme.active_title_color.get();
                self.fill_boxes2(slice::from_ref(aw), &c, x, y);
            }
            let c = theme.underline_color.get();
            self.fill_boxes2(slice::from_ref(&rd.underline), &c, x, y);
            let c = theme.title_color.get();
            self.fill_boxes2(&rd.inactive_workspaces, &c, x, y);
            for title in &rd.titles {
                self.render_texture(&title.tex, x + title.x, y + title.y, ARGB8888);
            }
            if let Some(status) = &rd.status {
                self.render_texture(&status.tex, x + status.x, y + status.y, ARGB8888);
            }
        }
        if let Some(ws) = output.workspace.get() {
            self.render_workspace(&ws, x, y + th + 1);
        }
        for stacked in self.state.root.stacked.iter() {
            if stacked.node_visible() {
                let pos = stacked.node_absolute_position();
                if pos.intersects(&opos) {
                    let (x, y) = opos.translate(pos.x1(), pos.y1());
                    stacked.node_render(self, x, y);
                }
            }
        }
        render_layer!(output.layers[2]);
        render_layer!(output.layers[3]);
    }

    pub fn render_workspace(&mut self, workspace: &WorkspaceNode, x: i32, y: i32) {
        if let Some(node) = workspace.container.get() {
            self.render_container(&node, x, y)
        }
    }

    fn x_to_f(&self, x: i32) -> f32 {
        2.0 * (x as f32 / self.fb.width as f32) - 1.0
    }

    fn y_to_f(&self, y: i32) -> f32 {
        2.0 * (y as f32 / self.fb.height as f32) - 1.0
    }

    fn fill_boxes(&self, boxes: &[Rect], color: &Color) {
        self.fill_boxes2(boxes, color, 0, 0);
    }

    fn fill_boxes2(&self, boxes: &[Rect], color: &Color, dx: i32, dy: i32) {
        if boxes.is_empty() {
            return;
        }
        let mut pos = Vec::with_capacity(boxes.len() * 12);
        for bx in boxes {
            let x1 = self.x_to_f(bx.x1() + dx);
            let y1 = self.y_to_f(bx.y1() + dy);
            let x2 = self.x_to_f(bx.x2() + dx);
            let y2 = self.y_to_f(bx.y2() + dy);
            pos.extend_from_slice(&[
                // triangle 1
                x2, y1, // top right
                x1, y1, // top left
                x1, y2, // bottom left
                // triangle 2
                x2, y1, // top right
                x1, y2, // bottom left
                x2, y2, // bottom right
            ]);
        }
        unsafe {
            glUseProgram(self.ctx.fill_prog.prog);
            glUniform4f(self.ctx.fill_prog_color, color.r, color.g, color.b, color.a);
            glVertexAttribPointer(
                self.ctx.fill_prog_pos as _,
                2,
                GL_FLOAT,
                GL_FALSE,
                0,
                pos.as_ptr() as _,
            );
            glEnableVertexAttribArray(self.ctx.fill_prog_pos as _);
            glDrawArrays(GL_TRIANGLES, 0, (boxes.len() * 6) as _);
            glDisableVertexAttribArray(self.ctx.fill_prog_pos as _);
        }
    }

    pub fn render_placeholder(&mut self, placeholder: &PlaceholderNode, x: i32, y: i32) {
        let pos = placeholder.position();
        self.fill_boxes(
            std::slice::from_ref(&pos.at_point(x, y)),
            &Color::from_rgba(20, 20, 20, 255),
        );
        if let Some(tex) = placeholder.texture() {
            let x = x + (pos.width() - tex.width()) / 2;
            let y = y + (pos.height() - tex.height()) / 2;
            log::info!("render at {}x{}", x, y);
            self.render_texture(&tex, x, y, &ARGB8888);
        }
    }

    pub fn render_container(&mut self, container: &ContainerNode, x: i32, y: i32) {
        {
            let rd = container.render_data.borrow_mut();
            let c = self.state.theme.title_color.get();
            self.fill_boxes2(&rd.title_rects, &c, x, y);
            let c = self.state.theme.active_title_color.get();
            self.fill_boxes2(&rd.active_title_rects, &c, x, y);
            let c = self.state.theme.underline_color.get();
            self.fill_boxes2(&rd.underline_rects, &c, x, y);
            let c = self.state.theme.border_color.get();
            self.fill_boxes2(&rd.border_rects, &c, x, y);
            if let Some(lar) = &rd.last_active_rect {
                let c = self.state.theme.last_active_color.get();
                self.fill_boxes2(std::slice::from_ref(lar), &c, x, y);
            }
            for title in &rd.titles {
                self.render_texture(&title.tex, x + title.x, y + title.y, ARGB8888);
            }
        }
        if let Some(child) = container.mono_child.get() {
            unsafe {
                let body = container.mono_body.get().move_(x, y);
                with_scissor(&body, || {
                    let content = container.mono_content.get();
                    child
                        .node
                        .node_render(self, x + content.x1(), y + content.y1());
                });
            }
        } else {
            for child in container.children.iter() {
                let body = child.body.get();
                if body.x1() >= container.width.get() || body.y1() >= container.height.get() {
                    break;
                }
                let body = body.move_(x, y);
                unsafe {
                    with_scissor(&body, || {
                        let content = child.content.get();
                        child
                            .node
                            .node_render(self, x + content.x1(), y + content.y1());
                    });
                }
            }
        }
    }

    pub fn render_xdg_surface(&mut self, xdg: &XdgSurface, mut x: i32, mut y: i32) {
        let surface = &xdg.surface;
        if let Some(geo) = xdg.geometry() {
            let (xt, yt) = geo.translate(x, y);
            x = xt;
            y = yt;
        }
        self.render_surface(surface, x, y);
    }

    pub fn render_surface(&mut self, surface: &WlSurface, x: i32, y: i32) {
        let children = surface.children.borrow();
        let buffer = match surface.buffer.get() {
            Some(b) => b,
            _ => {
                if !surface.is_cursor() {
                    log::warn!("surface has no buffer attached");
                }
                return;
            }
        };
        if let Some(children) = children.deref() {
            macro_rules! render {
                ($children:expr) => {
                    for child in $children.rev_iter() {
                        if child.pending.get() {
                            continue;
                        }
                        let pos = child.sub_surface.position.get();
                        self.render_surface(&child.sub_surface.surface, x + pos.x1(), y + pos.y1());
                    }
                };
            }
            render!(&children.below);
            self.render_buffer(&buffer, x, y);
            render!(&children.above);
        } else {
            self.render_buffer(&buffer, x, y);
        }
        let mut fr = surface.frame_requests.borrow_mut();
        for cb in fr.drain(..) {
            surface.client.dispatch_frame_requests.push(cb);
        }
    }

    pub fn render_buffer(&mut self, buffer: &WlBuffer, x: i32, y: i32) {
        if let Some(tex) = buffer.texture.get() {
            self.render_texture(&tex, x, y, buffer.format);
        }
    }

    pub fn render_texture(&mut self, texture: &Texture, x: i32, y: i32, format: &Format) {
        assert!(rc_eq(&self.ctx.ctx, &texture.ctx.ctx));
        unsafe {
            glActiveTexture(GL_TEXTURE0);

            glBindTexture(GL_TEXTURE_2D, texture.gl.tex);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);

            let prog = match format.has_alpha {
                true => {
                    glEnable(GL_BLEND);
                    &self.ctx.tex_alpha_prog
                }
                false => {
                    glDisable(GL_BLEND);
                    &self.ctx.tex_prog
                }
            };

            glUseProgram(prog.prog.prog);

            glUniform1i(prog.tex, 0);

            let texcoord: [f32; 8] = [1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0];

            let f_width = self.fb.width as f32;
            let f_height = self.fb.height as f32;

            let x1 = 2.0 * (x as f32 / f_width) - 1.0;
            let y1 = 2.0 * (y as f32 / f_height) - 1.0;
            let x2 = 2.0 * ((x + texture.gl.width) as f32 / f_width) - 1.0;
            let y2 = 2.0 * ((y + texture.gl.height) as f32 / f_height) - 1.0;

            let pos: [f32; 8] = [
                x2, y1, // top right
                x1, y1, // top left
                x2, y2, // bottom right
                x1, y2, // bottom left
            ];

            glVertexAttribPointer(
                prog.texcoord as _,
                2,
                GL_FLOAT,
                GL_FALSE,
                0,
                texcoord.as_ptr() as _,
            );
            glVertexAttribPointer(prog.pos as _, 2, GL_FLOAT, GL_FALSE, 0, pos.as_ptr() as _);

            glEnableVertexAttribArray(prog.texcoord as _);
            glEnableVertexAttribArray(prog.pos as _);

            glDrawArrays(GL_TRIANGLE_STRIP, 0, 4);

            glDisableVertexAttribArray(prog.texcoord as _);
            glDisableVertexAttribArray(prog.pos as _);

            glBindTexture(GL_TEXTURE_2D, 0);
        }
    }

    pub fn render_floating(&mut self, floating: &FloatNode, x: i32, y: i32) {
        let child = match floating.child.get() {
            Some(c) => c,
            _ => return,
        };
        let pos = floating.position.get();
        let theme = &self.state.theme;
        let th = theme.title_height.get();
        let bw = theme.border_width.get();
        let bc = theme.border_color.get();
        let tc = match floating.active.get() {
            true => theme.active_title_color.get(),
            false => theme.title_color.get(),
        };
        let uc = theme.underline_color.get();
        let borders = [
            Rect::new_sized(x, y, pos.width(), bw).unwrap(),
            Rect::new_sized(x, y + bw, bw, pos.height() - bw).unwrap(),
            Rect::new_sized(x + pos.width() - bw, y + bw, bw, pos.height() - bw).unwrap(),
            Rect::new_sized(x + bw, y + pos.height() - bw, pos.width() - 2 * bw, bw).unwrap(),
        ];
        self.fill_boxes(&borders, &bc);
        let title = [Rect::new_sized(x + bw, y + bw, pos.width() - 2 * bw, th).unwrap()];
        self.fill_boxes(&title, &tc);
        let title_underline =
            [Rect::new_sized(x + bw, y + bw + th, pos.width() - 2 * bw, 1).unwrap()];
        self.fill_boxes(&title_underline, &uc);
        if let Some(title) = floating.title_texture.get() {
            self.render_texture(&title, x + bw, y + bw, ARGB8888);
        }
        let body = Rect::new_sized(
            x + bw,
            y + bw + th + 1,
            pos.width() - 2 * bw,
            pos.height() - 2 * bw - th - 1,
        )
        .unwrap();
        unsafe {
            with_scissor(&body, || {
                child.node_render(self, body.x1(), body.y1());
            });
        }
    }

    pub fn render_layer_surface(&mut self, surface: &ZwlrLayerSurfaceV1, x: i32, y: i32) {
        unsafe {
            let body = surface.position().at_point(x, y);
            with_scissor(&body, || {
                self.render_surface(&surface.surface, x, y);
            });
        }
    }
}
