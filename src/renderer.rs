use {
    crate::{
        format::ARGB8888,
        gfx_api::{BufferPoints, GfxApiOpt},
        ifs::{
            wl_buffer::WlBuffer,
            wl_callback::WlCallback,
            wl_surface::{
                xdg_surface::XdgSurface, zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, WlSurface,
            },
            wp_presentation_feedback::WpPresentationFeedback,
        },
        rect::Rect,
        renderer::renderer_base::RendererBase,
        scale::Scale,
        state::State,
        theme::Color,
        tree::{
            ContainerNode, DisplayNode, FloatNode, OutputNode, PlaceholderNode, ToplevelNode,
            WorkspaceNode,
        },
    },
    std::{
        fmt::{Debug, Formatter},
        ops::Deref,
        rc::Rc,
        slice,
    },
};

pub mod renderer_base;

#[derive(Default)]
pub struct RenderResult {
    pub frame_requests: Vec<Rc<WlCallback>>,
    pub presentation_feedbacks: Vec<Rc<WpPresentationFeedback>>,
}

impl Debug for RenderResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderResult").finish_non_exhaustive()
    }
}

pub struct Renderer<'a> {
    pub base: RendererBase<'a>,
    pub state: &'a State,
    pub on_output: bool,
    pub result: &'a mut RenderResult,
    pub logical_extents: Rect,
    pub physical_extents: Rect,
}

impl Renderer<'_> {
    pub fn scale(&self) -> Scale {
        self.base.scale
    }

    pub fn physical_extents(&self) -> Rect {
        self.physical_extents
    }

    pub fn logical_extents(&self) -> Rect {
        self.logical_extents
    }

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
        if self.state.lock.locked.get() {
            if let Some(surface) = output.lock_surface.get() {
                if surface.surface.buffer.get().is_some() {
                    self.render_surface(&surface.surface, x, y, i32::MAX, i32::MAX);
                }
            }
            return;
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
        if let Some(ws) = output.workspace.get() {
            if let Some(fs) = ws.fullscreen.get() {
                fs.tl_as_node().node_render(self, x, y, i32::MAX, i32::MAX);
                render_layer!(output.layers[2]);
                render_layer!(output.layers[3]);
                return;
            }
        }
        render_layer!(output.layers[0]);
        render_layer!(output.layers[1]);
        let theme = &self.state.theme;
        let th = theme.sizes.title_height.get();
        {
            let c = theme.colors.bar_background.get();
            self.base.fill_boxes2(
                slice::from_ref(&Rect::new_sized(0, 0, opos.width(), th).unwrap()),
                &c,
                x,
                y,
            );
            let has_captures =
                !output.screencasts.is_empty() || !output.global.pending_captures.is_empty();
            let rd = output.render_data.borrow_mut();
            if let Some(aw) = &rd.active_workspace {
                let c = match has_captures && aw.captured {
                    true => theme.colors.captured_focused_title_background.get(),
                    false => theme.colors.focused_title_background.get(),
                };
                self.base.fill_boxes2(slice::from_ref(&aw.rect), &c, x, y);
            }
            let c = theme.colors.separator.get();
            self.base
                .fill_boxes2(slice::from_ref(&rd.underline), &c, x, y);
            let c = theme.colors.unfocused_title_background.get();
            self.base.fill_boxes2(&rd.inactive_workspaces, &c, x, y);
            let c = match has_captures {
                true => theme.colors.captured_unfocused_title_background.get(),
                false => theme.colors.unfocused_title_background.get(),
            };
            self.base
                .fill_boxes2(&rd.captured_inactive_workspaces, &c, x, y);
            let scale = output.preferred_scale.get();
            for title in &rd.titles {
                let (x, y) = self.base.scale_point(x + title.tex_x, y + title.tex_y);
                self.base.render_texture(
                    &title.tex,
                    x,
                    y,
                    ARGB8888,
                    None,
                    None,
                    scale,
                    i32::MAX,
                    i32::MAX,
                );
            }
            if let Some(status) = &rd.status {
                let (x, y) = self.base.scale_point(x + status.tex_x, y + status.tex_y);
                self.base.render_texture(
                    &status.tex,
                    x,
                    y,
                    ARGB8888,
                    None,
                    None,
                    scale,
                    i32::MAX,
                    i32::MAX,
                );
            }
        }
        if let Some(ws) = output.workspace.get() {
            self.render_workspace(&ws, x, y + th + 1);
        }
        for stacked in self.state.root.stacked.iter() {
            if stacked.node_visible() {
                self.base.ops.push(GfxApiOpt::Sync);
                let pos = stacked.node_absolute_position();
                if pos.intersects(&opos) {
                    let (x, y) = opos.translate(pos.x1(), pos.y1());
                    stacked.node_render(self, x, y, i32::MAX, i32::MAX);
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

    pub fn render_placeholder(&mut self, placeholder: &PlaceholderNode, x: i32, y: i32) {
        let pos = placeholder.tl_data().pos.get();
        self.base.fill_boxes(
            std::slice::from_ref(&pos.at_point(x, y)),
            &Color::from_rgba_straight(20, 20, 20, 255),
        );
        if let Some(tex) = placeholder.textures.get(&self.base.scale) {
            let x = x + (pos.width() - tex.width()) / 2;
            let y = y + (pos.height() - tex.height()) / 2;
            self.base.render_texture(
                &tex,
                x,
                y,
                ARGB8888,
                None,
                None,
                self.base.scale,
                i32::MAX,
                i32::MAX,
            );
        }
    }

    pub fn render_container(&mut self, container: &ContainerNode, x: i32, y: i32) {
        {
            let rd = container.render_data.borrow_mut();
            let c = self.state.theme.colors.unfocused_title_background.get();
            self.base.fill_boxes2(&rd.title_rects, &c, x, y);
            let c = self.state.theme.colors.focused_title_background.get();
            self.base.fill_boxes2(&rd.active_title_rects, &c, x, y);
            let c = self.state.theme.colors.separator.get();
            self.base.fill_boxes2(&rd.underline_rects, &c, x, y);
            let c = self.state.theme.colors.border.get();
            self.base.fill_boxes2(&rd.border_rects, &c, x, y);
            if let Some(lar) = &rd.last_active_rect {
                let c = self
                    .state
                    .theme
                    .colors
                    .focused_inactive_title_background
                    .get();
                self.base.fill_boxes2(std::slice::from_ref(lar), &c, x, y);
            }
            if let Some(titles) = rd.titles.get(&self.base.scale) {
                for title in titles {
                    let (x, y) = self.base.scale_point(x + title.x, y + title.y);
                    self.base.render_texture(
                        &title.tex,
                        x,
                        y,
                        ARGB8888,
                        None,
                        None,
                        self.base.scale,
                        i32::MAX,
                        i32::MAX,
                    );
                }
            }
        }
        if let Some(child) = container.mono_child.get() {
            let body = container.mono_body.get().move_(x, y);
            let body = self.base.scale_rect(body);
            let content = container.mono_content.get();
            child.node.node_render(
                self,
                x + content.x1(),
                y + content.y1(),
                body.width(),
                body.height(),
            );
        } else {
            for child in container.children.iter() {
                let body = child.body.get();
                if body.x1() >= container.width.get() || body.y1() >= container.height.get() {
                    break;
                }
                let body = body.move_(x, y);
                let body = self.base.scale_rect(body);
                let content = child.content.get();
                child.node.node_render(
                    self,
                    x + content.x1(),
                    y + content.y1(),
                    body.width(),
                    body.height(),
                );
            }
        }
    }

    pub fn render_xdg_surface(
        &mut self,
        xdg: &XdgSurface,
        mut x: i32,
        mut y: i32,
        max_width: i32,
        max_height: i32,
    ) {
        let surface = &xdg.surface;
        if let Some(geo) = xdg.geometry() {
            let (xt, yt) = geo.translate(x, y);
            x = xt;
            y = yt;
        }
        self.render_surface(surface, x, y, max_width, max_height);
    }

    pub fn render_surface(
        &mut self,
        surface: &WlSurface,
        x: i32,
        y: i32,
        max_width: i32,
        max_height: i32,
    ) {
        let (x, y) = self.base.scale_point(x, y);
        self.render_surface_scaled(surface, x, y, None, max_width, max_height);
    }

    pub fn render_surface_scaled(
        &mut self,
        surface: &WlSurface,
        x: i32,
        y: i32,
        pos_rel: Option<(i32, i32)>,
        max_width: i32,
        max_height: i32,
    ) {
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
        let tpoints = surface.buffer_points_norm.borrow_mut();
        let mut size = surface.buffer_abs_pos.get().size();
        if let Some((x_rel, y_rel)) = pos_rel {
            let (x, y) = self.base.scale_point(x_rel, y_rel);
            let (w, h) = self.base.scale_point(x_rel + size.0, y_rel + size.1);
            size = (w - x, h - y);
        } else {
            size = self.base.scale_point(size.0, size.1);
        }
        if let Some(children) = children.deref() {
            macro_rules! render {
                ($children:expr) => {
                    for child in $children.rev_iter() {
                        if child.pending.get() {
                            continue;
                        }
                        let pos = child.sub_surface.position.get();
                        let (x1, y1) = self.base.scale_point(pos.x1(), pos.y1());
                        self.render_surface_scaled(
                            &child.sub_surface.surface,
                            x + x1,
                            y + y1,
                            Some((pos.x1(), pos.y1())),
                            max_width,
                            max_height,
                        );
                    }
                };
            }
            render!(&children.below);
            self.render_buffer(&buffer, x, y, *tpoints, size, max_width, max_height);
            render!(&children.above);
        } else {
            self.render_buffer(&buffer, x, y, *tpoints, size, max_width, max_height);
        }
        if self.on_output {
            {
                let mut fr = surface.frame_requests.borrow_mut();
                self.result.frame_requests.extend(fr.drain(..));
            }
            {
                let mut fbs = surface.presentation_feedback.borrow_mut();
                self.result.presentation_feedbacks.extend(fbs.drain(..));
            }
        }
    }

    pub fn render_buffer(
        &mut self,
        buffer: &WlBuffer,
        x: i32,
        y: i32,
        tpoints: BufferPoints,
        tsize: (i32, i32),
        max_width: i32,
        max_height: i32,
    ) {
        if let Some(tex) = buffer.texture.get() {
            self.base.render_texture(
                &tex,
                x,
                y,
                buffer.format,
                Some(tpoints),
                Some(tsize),
                self.base.scale,
                max_width,
                max_height,
            );
        }
    }

    pub fn render_floating(&mut self, floating: &FloatNode, x: i32, y: i32) {
        let child = match floating.child.get() {
            Some(c) => c,
            _ => return,
        };
        let pos = floating.position.get();
        let theme = &self.state.theme;
        let th = theme.sizes.title_height.get();
        let bw = theme.sizes.border_width.get();
        let bc = theme.colors.border.get();
        let tc = match floating.active.get() {
            true => theme.colors.focused_title_background.get(),
            false => theme.colors.unfocused_title_background.get(),
        };
        let uc = theme.colors.separator.get();
        let borders = [
            Rect::new_sized(x, y, pos.width(), bw).unwrap(),
            Rect::new_sized(x, y + bw, bw, pos.height() - bw).unwrap(),
            Rect::new_sized(x + pos.width() - bw, y + bw, bw, pos.height() - bw).unwrap(),
            Rect::new_sized(x + bw, y + pos.height() - bw, pos.width() - 2 * bw, bw).unwrap(),
        ];
        self.base.fill_boxes(&borders, &bc);
        let title = [Rect::new_sized(x + bw, y + bw, pos.width() - 2 * bw, th).unwrap()];
        self.base.fill_boxes(&title, &tc);
        let title_underline =
            [Rect::new_sized(x + bw, y + bw + th, pos.width() - 2 * bw, 1).unwrap()];
        self.base.fill_boxes(&title_underline, &uc);
        if let Some(title) = floating.title_textures.get(&self.base.scale) {
            let (x, y) = self.base.scale_point(x + bw, y + bw);
            self.base.render_texture(
                &title,
                x,
                y,
                ARGB8888,
                None,
                None,
                self.base.scale,
                i32::MAX,
                i32::MAX,
            );
        }
        let body = Rect::new_sized(
            x + bw,
            y + bw + th + 1,
            pos.width() - 2 * bw,
            pos.height() - 2 * bw - th - 1,
        )
        .unwrap();
        let scissor_body = self.base.scale_rect(body);
        child.node_render(
            self,
            body.x1(),
            body.y1(),
            scissor_body.width(),
            scissor_body.height(),
        );
    }

    pub fn render_layer_surface(&mut self, surface: &ZwlrLayerSurfaceV1, x: i32, y: i32) {
        let body = surface.position().at_point(x, y);
        let body = self.base.scale_rect(body);
        self.render_surface(&surface.surface, x, y, body.width(), body.height());
    }
}
