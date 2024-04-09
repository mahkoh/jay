use {
    crate::{
        gfx_api::{AcquireSync, GfxApiOpt, ReleaseSync, SampleRect},
        ifs::{
            wl_callback::WlCallback,
            wl_surface::{
                xdg_surface::XdgSurface, zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, SurfaceBuffer,
                WlSurface,
            },
            wp_presentation_feedback::WpPresentationFeedback,
        },
        rect::Rect,
        renderer::renderer_base::RendererBase,
        scale::Scale,
        state::State,
        theme::Color,
        tree::{
            ContainerNode, DisplayNode, FloatNode, OutputNode, PlaceholderNode, ToplevelNodeBase,
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

impl RenderResult {
    pub fn dispatch_frame_requests(&mut self) {
        for fr in self.frame_requests.drain(..) {
            fr.send_done();
            let _ = fr.client.remove_obj(&*fr);
        }
    }
}

impl Debug for RenderResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderResult").finish_non_exhaustive()
    }
}

pub struct Renderer<'a> {
    pub base: RendererBase<'a>,
    pub state: &'a State,
    pub result: Option<&'a mut RenderResult>,
    pub logical_extents: Rect,
    pub pixel_extents: Rect,
}

impl Renderer<'_> {
    pub fn scale(&self) -> Scale {
        self.base.scale
    }

    pub fn pixel_extents(&self) -> Rect {
        self.pixel_extents
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
                if surface.surface.buffer.is_some() {
                    self.render_surface(&surface.surface, x, y, None);
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
                    self.base.ops.push(GfxApiOpt::Sync);
                }
            };
        }
        let mut fullscreen = None;
        if let Some(ws) = output.workspace.get() {
            fullscreen = ws.fullscreen.get();
        }
        if let Some(fs) = fullscreen {
            fs.tl_as_node().node_render(self, x, y, None);
        } else {
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
                let c = theme.colors.attention_requested_background.get();
                self.base
                    .fill_boxes2(&rd.attention_requested_workspaces, &c, x, y);
                let scale = output.global.persistent.scale.get();
                for title in &rd.titles {
                    let (x, y) = self.base.scale_point(x + title.tex_x, y + title.tex_y);
                    self.base.render_texture(
                        &title.tex,
                        None,
                        x,
                        y,
                        None,
                        None,
                        scale,
                        None,
                        None,
                        AcquireSync::None,
                        ReleaseSync::None,
                    );
                }
                if let Some(status) = &rd.status {
                    let (x, y) = self.base.scale_point(x + status.tex_x, y + status.tex_y);
                    self.base.render_texture(
                        &status.tex.texture,
                        None,
                        x,
                        y,
                        None,
                        None,
                        scale,
                        None,
                        None,
                        AcquireSync::None,
                        ReleaseSync::None,
                    );
                }
            }
            if let Some(ws) = output.workspace.get() {
                self.render_workspace(&ws, x, y + th + 1);
            }
        }
        for stacked in self.state.root.stacked.iter() {
            if stacked.node_visible() {
                self.base.ops.push(GfxApiOpt::Sync);
                let pos = stacked.node_absolute_position();
                if pos.intersects(&opos) {
                    let (x, y) = opos.translate(pos.x1(), pos.y1());
                    stacked.node_render(self, x, y, None);
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
            let (tex_width, tex_height) = tex.texture.size();
            let x = x + (pos.width() - tex_width) / 2;
            let y = y + (pos.height() - tex_height) / 2;
            self.base.render_texture(
                &tex.texture,
                None,
                x,
                y,
                None,
                None,
                self.base.scale,
                None,
                None,
                AcquireSync::None,
                ReleaseSync::None,
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
            let c = self.state.theme.colors.attention_requested_background.get();
            self.base.fill_boxes2(&rd.attention_title_rects, &c, x, y);
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
                        &title.tex.texture,
                        None,
                        x,
                        y,
                        None,
                        None,
                        self.base.scale,
                        None,
                        None,
                        AcquireSync::None,
                        ReleaseSync::None,
                    );
                }
            }
        }
        if let Some(child) = container.mono_child.get() {
            let body = container.mono_body.get().move_(x, y);
            let body = self.base.scale_rect(body);
            let content = container.mono_content.get();
            child
                .node
                .node_render(self, x + content.x1(), y + content.y1(), Some(&body));
        } else {
            for child in container.children.iter() {
                let body = child.body.get();
                if body.x1() >= container.width.get() || body.y1() >= container.height.get() {
                    break;
                }
                let body = body.move_(x, y);
                let body = self.base.scale_rect(body);
                let content = child.content.get();
                child
                    .node
                    .node_render(self, x + content.x1(), y + content.y1(), Some(&body));
            }
        }
    }

    pub fn render_xdg_surface(
        &mut self,
        xdg: &XdgSurface,
        mut x: i32,
        mut y: i32,
        bounds: Option<&Rect>,
    ) {
        let surface = &xdg.surface;
        if let Some(geo) = xdg.geometry() {
            (x, y) = geo.translate(x, y);
        }
        self.render_surface(surface, x, y, bounds);
    }

    pub fn render_surface(&mut self, surface: &WlSurface, x: i32, y: i32, bounds: Option<&Rect>) {
        let (x, y) = self.base.scale_point(x, y);
        self.render_surface_scaled(surface, x, y, None, bounds, false);
    }

    pub fn render_surface_scaled(
        &mut self,
        surface: &WlSurface,
        x: i32,
        y: i32,
        pos_rel: Option<(i32, i32)>,
        bounds: Option<&Rect>,
        is_subsurface: bool,
    ) {
        let children = surface.children.borrow();
        let buffer = match surface.buffer.get() {
            Some(b) => b,
            _ => {
                if !surface.is_cursor() && !is_subsurface {
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
        let alpha = surface.alpha();
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
                            bounds,
                            true,
                        );
                    }
                };
            }
            render!(&children.below);
            self.render_buffer(&buffer, alpha, x, y, *tpoints, size, bounds);
            render!(&children.above);
        } else {
            self.render_buffer(&buffer, alpha, x, y, *tpoints, size, bounds);
        }
        if let Some(result) = self.result.as_deref_mut() {
            {
                let mut fr = surface.frame_requests.borrow_mut();
                result.frame_requests.extend(fr.drain(..));
            }
            {
                let mut fbs = surface.presentation_feedback.borrow_mut();
                result.presentation_feedbacks.extend(fbs.drain(..));
            }
        }
    }

    pub fn render_buffer(
        &mut self,
        buffer: &Rc<SurfaceBuffer>,
        alpha: Option<f32>,
        x: i32,
        y: i32,
        tpoints: SampleRect,
        tsize: (i32, i32),
        bounds: Option<&Rect>,
    ) {
        if let Some(tex) = buffer.buffer.texture.get() {
            self.base.render_texture(
                &tex,
                alpha,
                x,
                y,
                Some(tpoints),
                Some(tsize),
                self.base.scale,
                bounds,
                Some(buffer.clone()),
                buffer.sync.clone(),
                buffer.release_sync,
            );
        } else if let Some(color) = &buffer.buffer.color {
            if let Some(rect) = Rect::new_sized(x, y, tsize.0, tsize.1) {
                let rect = match bounds {
                    None => rect,
                    Some(bounds) => rect.intersect(*bounds),
                };
                if !rect.is_empty() {
                    self.base.ops.push(GfxApiOpt::Sync);
                    let mut color = *color;
                    if let Some(alpha) = alpha {
                        color = color * alpha;
                    }
                    self.base.fill_boxes(&[rect], &color);
                }
            }
        } else {
            log::info!("live buffer has neither a texture nor is a single-pixel buffer");
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
        let tc = if floating.active.get() {
            theme.colors.focused_title_background.get()
        } else if floating.attention_requested.get() {
            theme.colors.attention_requested_background.get()
        } else {
            theme.colors.unfocused_title_background.get()
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
                &title.texture,
                None,
                x,
                y,
                None,
                None,
                self.base.scale,
                None,
                None,
                AcquireSync::None,
                ReleaseSync::None,
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
        child.node_render(self, body.x1(), body.y1(), Some(&scissor_body));
    }

    pub fn render_layer_surface(&mut self, surface: &ZwlrLayerSurfaceV1, x: i32, y: i32) {
        let body = surface.position().at_point(x, y);
        let body = self.base.scale_rect(body);
        self.render_surface(&surface.surface, x, y, Some(&body));
    }
}
