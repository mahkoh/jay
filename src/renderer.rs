use crate::cmm::cmm_render_intent::RenderIntent;
use crate::gfx_api::AcquireSync;
use crate::gfx_api::BufferResv;
use crate::gfx_api::GfxApiOp;
use crate::gfx_api::GfxTexture;
use crate::gfx_api::LazyTexture;
use crate::gfx_api::ReleaseSync;
use crate::gfx_api::SampleRect;
use crate::icons::IconState;
use crate::icons::SizedBarIcons;
use crate::icons::SizedTitleIcons;
use crate::ifs::wl_surface::SurfaceBuffer;
use crate::ifs::wl_surface::WlSurface;
use crate::ifs::wl_surface::x_surface::xwindow::Xwindow;
use crate::ifs::wl_surface::xdg_surface::XdgSurface;
use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::XdgToplevel;
use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::xdg_toplevel_icon_v1::ToplevelIcon;
use crate::ifs::wl_surface::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1;
use crate::rect::Rect;
use crate::renderer::renderer_base::RenderTexture;
use crate::renderer::renderer_base::RendererBase;
use crate::scale::Scale;
use crate::state::State;
use crate::theme::Color;
use crate::tree::ContainerChildType;
use crate::tree::ContainerNode;
use crate::tree::DisplayNode;
use crate::tree::FloatNode;
use crate::tree::NodeBase;
use crate::tree::OutputNode;
use crate::tree::PlaceholderNode;
use crate::tree::ToplevelData;
use crate::tree::ToplevelNodeBase;
use crate::tree::TreeTimeline::RenderTL;
use crate::tree::WorkspaceNode;
use crate::tree::WorkspaceType;
use std::ops::Deref;
use std::rc::Rc;
use std::slice;

pub mod renderer_base;

pub struct Renderer<'a> {
    pub base: RendererBase<'a>,
    pub state: &'a State,
    pub logical_extents: Rect,
    pub pixel_extents: Rect,
    pub title_icons: Option<Rc<SizedTitleIcons>>,
    pub bar_icons: Option<Rc<SizedBarIcons>>,
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
        let ext = display.node_state[RenderTL].extents.get();
        let outputs = display.outputs.lock();
        for output in outputs.values() {
            let opos = output.node_state[RenderTL].pos.get();
            let (ox, oy) = ext.translate(opos.x1(), opos.y1());
            self.render_output(output, x + ox, y + oy);
        }
    }

    pub fn render_output(&mut self, output: &OutputNode, x: i32, y: i32) {
        let ns = &output.node_state[RenderTL];
        if self.state.lock.locked[RenderTL].get() {
            if let Some(surface) = ns.lock_surface.get()
                && surface.surface.buffer.is_some()
            {
                self.render_surface(&surface.surface, x, y, None);
            }
            return;
        }
        let opos = ns.pos.get();
        macro_rules! render_layer {
            ($layer:expr) => {
                for ls in $layer.iter_valid(RenderTL) {
                    let pos = ls.output_extents();
                    self.render_layer_surface(ls.deref(), x + pos.x1(), y + pos.y1());
                    self.base.ops.push(GfxApiOp::Sync);
                }
            };
        }
        let mut fullscreen = None;
        let mut fullscreen_is_overlay = false;
        if let Some(ws) = ns.overlay.get() {
            let wns = &ws.node_state[RenderTL];
            fullscreen = wns.fullscreen.get();
            fullscreen_is_overlay = wns.fullscreen.is_some();
        }
        if fullscreen.is_none()
            && let Some(ws) = ns.workspace.get()
        {
            fullscreen = ws.node_state[RenderTL].fullscreen.get();
        }
        let theme = &self.state.theme;
        let srgb_srgb = self.state.color_manager.srgb_gamma22();
        let srgb = &srgb_srgb.linear;
        let perceptual = RenderIntent::Perceptual;
        if let Some(fs) = &fullscreen {
            if !fullscreen_is_overlay {
                fs.node_render(self, x, y, None);
            }
        } else {
            render_layer!(output.layers[0]);
            render_layer!(output.layers[1]);
            let ws = ns.workspace.get();
            if self.state.show_bar.get() {
                let non_exclusive_rect_rel = ns.rects.non_exclusive_rel.get();
                let (mut x, mut y) = non_exclusive_rect_rel.translate_inv(x, y);
                let bar_rect = ns.rects.bar_rel.get();
                let bar_bg = bar_rect.move_(
                    x - non_exclusive_rect_rel.x1(),
                    y - non_exclusive_rect_rel.y1(),
                );
                let bar_bg = self.base.scale_rect(bar_bg);
                let c = theme.colors.bar_background.get();
                self.base
                    .fill_scaled_boxes(slice::from_ref(&bar_bg), &c, None, srgb, perceptual);
                self.base.sync();
                let rd = output.render_data.borrow_mut();
                if let Some(aw) = &rd.active_workspace {
                    let c = match aw.captured {
                        true => theme.colors.captured_focused_title_background.get(),
                        false => theme.colors.focused_title_background.get(),
                    };
                    self.base
                        .fill_boxes2(slice::from_ref(&aw.rect), &c, srgb, perceptual, x, y);
                }
                if let Some(aw) = &rd.overlay_workspace {
                    self.base.fill_boxes2(
                        slice::from_ref(aw),
                        &theme.colors.focused_title_background.get(),
                        srgb,
                        perceptual,
                        x,
                        y,
                    );
                }
                let mut c = theme.colors.separator.get();
                if let Some(ws) = &ws
                    && ws.seat_state.is_active()
                {
                    c = theme.colors.focused_title_background.get();
                }
                self.base.fill_boxes2(
                    slice::from_ref(&rd.bar_separator),
                    &c,
                    srgb,
                    perceptual,
                    x,
                    y,
                );
                let c = theme.colors.unfocused_title_background.get();
                self.base
                    .fill_boxes2(&rd.inactive_workspaces, &c, srgb, perceptual, x, y);
                let c = theme.colors.captured_unfocused_title_background.get();
                self.base
                    .fill_boxes2(&rd.captured_inactive_workspaces, &c, srgb, perceptual, x, y);
                self.base.sync();
                let c = theme.colors.attention_requested_background.get();
                self.base.fill_boxes2(
                    &rd.attention_requested_workspaces,
                    &c,
                    srgb,
                    perceptual,
                    x,
                    y,
                );
                let scale = output.node_state[RenderTL].scale.get();
                for title in &rd.titles {
                    if let Some(icon_x) = title.icon_x
                        && let Some(icons) = &self.bar_icons
                    {
                        let (x, y) = self.base.scale_point(x + icon_x, y + title.tex_y);
                        self.base.render_texture(
                            &icons.overlay,
                            x,
                            y,
                            RenderTexture {
                                tscale: Some(scale),
                                bounds: Some(&bar_bg),
                                ..Default::default()
                            },
                        );
                    }
                    let (x, y) = self.base.scale_point(x + title.tex_x, y + title.tex_y);
                    self.base.render_texture(
                        &title.tex,
                        x,
                        y,
                        RenderTexture {
                            tscale: Some(scale),
                            bounds: Some(&bar_bg),
                            ..Default::default()
                        },
                    );
                }
                x += bar_rect.x1() - non_exclusive_rect_rel.x1();
                y += bar_rect.y1() - non_exclusive_rect_rel.y1();
                if let Some(status) = &rd.status
                    && let Some(texture) = status.tex.texture()
                {
                    let (x, y) = self.base.scale_point(x + status.tex_x, y);
                    self.base.render_texture(
                        &texture,
                        x,
                        y,
                        RenderTexture {
                            tscale: Some(scale),
                            bounds: Some(&bar_bg),
                            ..Default::default()
                        },
                    );
                }
                for item in output.tray_items.iter_valid(RenderTL) {
                    let data = item.data();
                    if data.surface.buffer.is_some() {
                        let rect = data.rel_pos[RenderTL].get().move_(x, y);
                        let bounds = self.base.scale_rect(rect);
                        self.render_surface(&data.surface, rect.x1(), rect.y1(), Some(&bounds));
                    }
                }
            }
            if let Some(ws) = &ws {
                let ws_rect = ns.rects.workspace_rel.get();
                let (x, y) = ws_rect.translate_inv(x, y);
                self.render_workspace(&ws, x, y);
            }
        }
        macro_rules! render_stacked {
            ($stack:expr) => {
                for stacked in $stack.iter_visible(RenderTL) {
                    self.base.sync();
                    let pos = stacked.node_absolute_position(RenderTL);
                    if pos.intersects(&opos) {
                        let (x, y) = opos.translate(pos.x1(), pos.y1());
                        stacked.node_render(self, x, y, None);
                    }
                }
            };
        }
        render_stacked!(self.state.root.stacked);
        if fullscreen.is_none() {
            render_layer!(output.layers[2]);
        }
        if !fullscreen_is_overlay {
            render_layer!(output.layers[3]);
        }
        render_stacked!(self.state.root.stacked_above_layers);
        if let Some(fs) = &fullscreen
            && fullscreen_is_overlay
        {
            fs.node_render(self, x, y, None);
        } else if let Some(ws) = ns.overlay.get() {
            let ws_rect = ns.rects.workspace_rel.get();
            let (x, y) = ws_rect.translate_inv(x, y);
            self.base.sync();
            self.render_workspace(&ws, x, y);
        }
        render_stacked!(self.state.root.stacked_in_overlay);
        for layer in [&ns.workspace, &ns.overlay] {
            if let Some(ws) = layer.get()
                && ws.render_highlight.get() > 0
            {
                let color = self.state.theme.colors.highlight.get();
                let bounds = ns.rects.workspace_rel.get().move_(x, y);
                self.base.sync();
                self.base.fill_boxes(&[bounds], &color, srgb, perceptual);
            }
        }
    }

    pub fn render_workspace(&mut self, workspace: &WorkspaceNode, x: i32, y: i32) {
        if let Some(node) = workspace.node_state[RenderTL].container.get() {
            self.render_container(&node, x, y)
        }
    }

    pub fn render_placeholder(
        &mut self,
        placeholder: &PlaceholderNode,
        x: i32,
        y: i32,
        bounds: Option<&Rect>,
    ) {
        let pos = placeholder.tl_data().content_size.get();
        self.base.fill_boxes(
            std::slice::from_ref(&pos.at_point(x, y)),
            &Color::from_srgba_straight(20, 20, 20, 255),
            &self.state.color_manager.srgb_gamma22().linear,
            RenderIntent::Perceptual,
        );
        if let Some(tex) = placeholder.textures.borrow().get(&self.base.scale)
            && let Some(texture) = tex.texture()
        {
            let (tex_width, tex_height) = texture.size();
            let x = x + (pos.width() - tex_width) / 2;
            let y = y + (pos.height() - tex_height) / 2;
            self.base.render_texture(
                &texture,
                x,
                y,
                RenderTexture {
                    bounds,
                    ..Default::default()
                },
            );
        }
        self.render_tl_aux(placeholder.tl_data(), bounds, true);
    }

    pub fn render_container(&mut self, container: &ContainerNode, x: i32, y: i32) {
        {
            let srgb_srgb = self.state.color_manager.srgb_gamma22();
            let srgb = &srgb_srgb.linear;
            let perceptual = RenderIntent::Perceptual;
            let rd = container.render_data.borrow_mut();
            let c = self.state.theme.colors.unfocused_title_background.get();
            self.base
                .fill_boxes2(&rd.title_rects, &c, srgb, perceptual, x, y);
            let c = self.state.theme.colors.focused_title_background.get();
            self.base
                .fill_boxes2(&rd.active_title_rects, &c, srgb, perceptual, x, y);
            let c = self.state.theme.colors.attention_requested_background.get();
            self.base
                .fill_boxes2(&rd.attention_title_rects, &c, srgb, perceptual, x, y);
            let c = self.state.theme.colors.separator.get();
            self.base
                .fill_boxes2(&rd.underline_rects, &c, srgb, perceptual, x, y);
            let c = self.state.theme.focused_border_color();
            self.base
                .fill_boxes2(&rd.active_border_rects, &c, srgb, perceptual, x, y);
            let c = self.state.theme.colors.border.get();
            self.base
                .fill_boxes2(&rd.border_rects, &c, srgb, perceptual, x, y);
            if let Some(lar) = &rd.last_active_rect {
                let c = self
                    .state
                    .theme
                    .colors
                    .focused_inactive_title_background
                    .get();
                self.base
                    .fill_boxes2(std::slice::from_ref(lar), &c, srgb, perceptual, x, y);
            }
            let draw_overlay_icon = container.tl_data().is_overlay_root_container.get();
            let th = self.state.theme.title_height(RenderTL);
            if let Some(titles) = rd.titles.get(&self.base.scale) {
                for title in titles {
                    let rect = title.rect.move_(x, y);
                    let bounds = self.base.scale_rect(rect);
                    let mut x = rect.x1();
                    if draw_overlay_icon {
                        if let Some(icons) = &self.title_icons {
                            let (x, y) = self.base.scale_point(x, rect.y1());
                            let icon = match title.ty {
                                ContainerChildType::Active => &icons.overlay_focused_title,
                                ContainerChildType::AttentionRequested => {
                                    &icons.overlay_attention_requested
                                }
                                ContainerChildType::LastActive => {
                                    &icons.overlay_focused_inactive_title
                                }
                                ContainerChildType::Other => &icons.overlay_unfocused_title,
                            };
                            self.base.render_texture(
                                icon,
                                x,
                                y,
                                RenderTexture {
                                    bounds: Some(&bounds),
                                    ..Default::default()
                                },
                            );
                        }
                        x += th;
                    }
                    if let Some(icon) = &title.icon {
                        self.render_icon(icon, &bounds, x, rect.y1());
                        x += th;
                    }
                    if let Some(tex) = &title.tex {
                        let (x, y) = self.base.scale_point(x, rect.y1());
                        self.base.render_texture(
                            tex,
                            x,
                            y,
                            RenderTexture {
                                bounds: Some(&bounds),
                                ..Default::default()
                            },
                        );
                    }
                }
            }
        }
        let ns = &container.node_state[RenderTL];
        if let Some(child) = ns.mono_child.get() {
            let body = ns.mono_body.get().move_(x, y);
            let body = self.base.scale_rect(body);
            let content = ns.mono_content.get();
            child
                .node
                .node_render(self, x + content.x1(), y + content.y1(), Some(&body));
        } else {
            for child in container.children.iter_valid(RenderTL) {
                let cns = &child.node_state[RenderTL];
                let body = cns.body.get();
                if body.x1() >= ns.width.get() || body.y1() >= ns.height.get() {
                    break;
                }
                let body = body.move_(x, y);
                let body = self.base.scale_rect(body);
                let content = cns.content.get();
                child
                    .node
                    .node_render(self, x + content.x1(), y + content.y1(), Some(&body));
            }
        }
        self.render_tl_aux(container.tl_data(), None, false);
    }

    pub fn render_xwindow(&mut self, tl: &Xwindow, x: i32, y: i32, bounds: Option<&Rect>) {
        self.render_surface(&tl.x.surface, x, y, bounds);
        self.render_tl_aux(tl.tl_data(), bounds, true);
    }

    pub fn render_xdg_toplevel(&mut self, tl: &XdgToplevel, x: i32, y: i32, bounds: Option<&Rect>) {
        self.render_xdg_surface(&tl.xdg, x, y, bounds);
        self.render_tl_aux(tl.tl_data(), bounds, true);
    }

    pub fn render_xdg_surface(
        &mut self,
        xdg: &XdgSurface,
        mut x: i32,
        mut y: i32,
        bounds: Option<&Rect>,
    ) {
        let surface = &xdg.surface;
        let geo = xdg.geometry(RenderTL);
        (x, y) = geo.translate(x, y);
        self.render_surface(surface, x, y, bounds);
    }

    fn render_tl_aux(
        &mut self,
        tl_data: &ToplevelData,
        bounds: Option<&Rect>,
        render_highlight: bool,
    ) {
        if render_highlight {
            self.render_tl_highlight(tl_data, bounds);
        }
    }

    fn render_tl_highlight(&mut self, tl_data: &ToplevelData, bounds: Option<&Rect>) {
        if tl_data.render_highlight.get() == 0 {
            return;
        }
        let Some(bounds) = bounds else {
            return;
        };
        let color = self.state.theme.colors.highlight.get();
        self.base.sync();
        self.base.fill_scaled_boxes(
            slice::from_ref(bounds),
            &color,
            None,
            &self.state.color_manager.srgb_gamma22().linear,
            RenderIntent::Perceptual,
        );
    }

    pub fn render_highlight(&mut self, rect: &Rect) {
        let color = self.state.theme.colors.highlight.get();
        self.base.sync();
        self.base.fill_boxes(
            slice::from_ref(rect),
            &color,
            &self.state.color_manager.srgb_gamma22().linear,
            RenderIntent::Perceptual,
        );
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
        if surface.buffer.is_none() {
            if !surface.is_cursor() && !is_subsurface {
                log::warn!("surface has no buffer attached");
            }
            return;
        }
        if !surface.node_visible(RenderTL) {
            log::warn!("node is invisible");
        }
        let tpoints = surface.buffer_points_norm.borrow_mut();
        let mut size = surface.buffer_abs_pos[RenderTL].get().size();
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
                    for child in $children.iter() {
                        if child.pending.get() {
                            continue;
                        }
                        let pos = child.sub_surface.position.get();
                        let (x1, y1) = self.base.scale_point(pos.0, pos.1);
                        self.render_surface_scaled(
                            &child.sub_surface.surface,
                            x + x1,
                            y + y1,
                            Some(pos),
                            bounds,
                            true,
                        );
                    }
                };
            }
            render!(&children.below);
            self.render_buffer(surface, x, y, *tpoints, size, bounds);
            render!(&children.above);
        } else {
            self.render_buffer(surface, x, y, *tpoints, size, bounds);
        }
    }

    pub fn render_buffer(
        &mut self,
        surface: &WlSurface,
        x: i32,
        y: i32,
        tpoints: SampleRect,
        tsize: (i32, i32),
        bounds: Option<&Rect>,
    ) {
        let alpha = surface.alpha();
        let cd = surface.color_description();
        let intent = surface.render_intent();
        let alpha_mode = surface.alpha_mode();
        let render_texture = |slf: &mut Renderer,
                              tex: &Rc<dyn GfxTexture>,
                              buffer: Rc<dyn BufferResv>,
                              release_sync: ReleaseSync,
                              client_buf: Option<Rc<SurfaceBuffer>>,
                              lazy: Option<Rc<dyn LazyTexture>>| {
            let mut opaque = surface.opaque();
            if !opaque && tex.format().has_alpha {
                opaque = slf.bounds_are_opaque(x, y, bounds, surface);
            }
            slf.base.render_texture(
                tex,
                x,
                y,
                RenderTexture {
                    alpha,
                    tpoints: Some(tpoints),
                    tsize: Some(tsize),
                    bounds,
                    buffer_resv: Some(buffer),
                    acquire_sync: AcquireSync::Unnecessary,
                    release_sync,
                    opaque,
                    cd: Some(&cd),
                    render_intent: intent,
                    alpha_mode,
                    client_buf,
                    lazy,
                    ..Default::default()
                },
            );
        };
        let Some(buffer) = surface.buffer.get() else {
            log::info!("surface has no client buffer");
            return;
        };
        let buf = &buffer.buffer.buf;
        if let Some(prime) = surface.prime.buffer() {
            render_texture(
                self,
                &prime.tex(),
                prime.clone(),
                ReleaseSync::Explicit,
                Some(buffer),
                Some(prime.clone()),
            );
        } else {
            if let Some(tex) = buf.get_texture(surface) {
                render_texture(self, &tex, buffer.clone(), buffer.release_sync, None, None);
            } else if let Some(color) = &buf.color {
                if let Some(rect) = Rect::new_sized(x, y, tsize.0, tsize.1) {
                    let rect = match bounds {
                        None => rect,
                        Some(bounds) => rect.intersect(*bounds),
                    };
                    if !rect.is_empty() {
                        let color = Color::from_u32(
                            cd.eotf, alpha_mode, color[0], color[1], color[2], color[3],
                        );
                        self.base.sync();
                        self.base
                            .fill_scaled_boxes(&[rect], &color, alpha, &cd.linear, intent);
                    }
                }
            } else {
                log::info!("client buffer has neither a texture nor is a single-pixel buffer");
            }
        }
    }

    pub fn render_floating(&mut self, floating: &FloatNode, x: i32, y: i32) {
        let ns = &floating.node_state[RenderTL];
        let child = match ns.child.get() {
            Some(c) => c,
            _ => return,
        };
        let pos = ns.position.get();
        let theme = &self.state.theme;
        let th = theme.title_height(RenderTL);
        let tpuh = theme.title_plus_underline_height(RenderTL);
        let tuh = theme.title_underline_height(RenderTL);
        let bw = theme.sizes.border_width.get(RenderTL);
        let bc = match ns.active.get() {
            true => theme.focused_border_color(),
            false => theme.colors.border.get(),
        };
        let tc = if ns.active.get() {
            theme.colors.focused_title_background.get()
        } else if ns.attention_requested.get() {
            theme.colors.attention_requested_background.get()
        } else {
            theme.colors.unfocused_title_background.get()
        };
        let uc = theme.colors.separator.get();
        let borders = [
            Rect::new_sized_saturating(x, y, pos.width(), bw),
            Rect::new_sized_saturating(x, y + bw, bw, pos.height() - bw),
            Rect::new_sized_saturating(x + pos.width() - bw, y + bw, bw, pos.height() - bw),
            Rect::new_sized_saturating(x + bw, y + pos.height() - bw, pos.width() - 2 * bw, bw),
        ];
        let srgb_srgb = self.state.color_manager.srgb_gamma22();
        let srgb = &srgb_srgb.linear;
        let perceptual = RenderIntent::Perceptual;
        self.base.fill_boxes(&borders, &bc, srgb, perceptual);
        let title = [Rect::new_sized_saturating(
            x + bw,
            y + bw,
            pos.width() - 2 * bw,
            th,
        )];
        self.base.fill_boxes(&title, &tc, srgb, perceptual);
        let title_underline = [Rect::new_sized_saturating(
            x + bw,
            y + bw + th,
            pos.width() - 2 * bw,
            tuh,
        )];
        self.base
            .fill_boxes(&title_underline, &uc, srgb, perceptual);
        let rect = ns.title_rect.get().move_(x, y);
        let bounds = self.base.scale_rect(rect);
        let (mut x1, y1) = rect.position();
        if ns.workspace_ty.get() == WorkspaceType::Overlay {
            if let Some(icons) = &self.title_icons {
                let icon = if ns.active.get() {
                    &icons.overlay_focused_title
                } else if ns.attention_requested.get() {
                    &icons.overlay_attention_requested
                } else {
                    &icons.overlay_unfocused_title
                };
                let (x, y) = self.base.scale_point(x1, y1);
                self.base.render_texture(
                    icon,
                    x,
                    y,
                    RenderTexture {
                        bounds: Some(&bounds),
                        ..Default::default()
                    },
                );
            }
            x1 += th;
        }
        let is_pinned = ns.pinned.get();
        if is_pinned || self.state.show_pin_icon.get() {
            let (x, y) = self.base.scale_point(x1, y1);
            if let Some(icons) = &self.title_icons {
                let icon = if ns.active.get() {
                    &icons.pin_focused_title
                } else if ns.attention_requested.get() {
                    &icons.pin_attention_requested
                } else {
                    &icons.pin_unfocused_title
                };
                let state = match is_pinned {
                    true => IconState::Active,
                    false => IconState::Passive,
                };
                self.base.render_texture(
                    &icon[state],
                    x,
                    y,
                    RenderTexture {
                        bounds: Some(&bounds),
                        ..Default::default()
                    },
                );
            }
            x1 += th;
        }
        if let Some(icon) = floating.icons.get(&self.base.scale) {
            self.render_icon(&icon, &bounds, x1, y1);
            x1 += th;
        }
        if let Some(title) = floating.title_textures.borrow().get(&self.base.scale)
            && let Some(texture) = title.texture()
        {
            let (x, y) = self.base.scale_point(x1, y1);
            self.base.render_texture(
                &texture,
                x,
                y,
                RenderTexture {
                    bounds: Some(&bounds),
                    ..Default::default()
                },
            );
        }
        let body = Rect::new_sized_saturating(
            x + bw,
            y + bw + tpuh,
            pos.width() - 2 * bw,
            pos.height() - 2 * bw - tpuh,
        );
        let scissor_body = self.base.scale_rect(body);
        child.node_render(self, body.x1(), body.y1(), Some(&scissor_body));
    }

    pub fn render_layer_surface(&mut self, surface: &ZwlrLayerSurfaceV1, x: i32, y: i32) {
        let (dx, dy) = surface.surface.extents.get().position();
        self.render_surface(&surface.surface, x - dx, y - dy, None);
    }

    fn bounds_are_opaque(
        &self,
        x: i32,
        y: i32,
        bounds: Option<&Rect>,
        surface: &WlSurface,
    ) -> bool {
        let Some(bounds) = bounds else {
            return false;
        };
        let Some(region) = surface.opaque_region() else {
            return false;
        };
        let surface_size = surface.buffer_abs_pos[RenderTL].get().at_point(0, 0);
        let surface_size = self.base.scale_rect(surface_size);
        let bounds = bounds.move_(-x, -y).intersect(surface_size);
        region.contains_rect2(&bounds, |r| self.base.scale_rect(*r))
    }

    fn render_icon(&mut self, icon: &ToplevelIcon, bounds: &Rect, x1: i32, y1: i32) {
        let (x, y) = self.base.scale_point(x1 + 1, y1 + 1);
        let grayscale = self.state.theme.window_icons_grayscale.get();
        let srgb = self.state.color_manager.srgb_gamma22();
        let perceptual = RenderIntent::Perceptual;
        match icon {
            ToplevelIcon::Srgb(color) => {
                let tis = self.state.theme.title_icon_size(RenderTL) + 1;
                let (x2, y2) = self.base.scale_point(x1 + tis, y1 + tis);
                let color = match grayscale {
                    true => color.to_grayscale(),
                    false => *color,
                };
                self.base.fill_scaled_boxes(
                    slice::from_ref(&Rect::new_saturating(x, y, x2, y2)),
                    &color,
                    None,
                    &srgb.linear,
                    perceptual,
                )
            }
            ToplevelIcon::Tex(tex) => {
                self.base.render_texture(
                    &tex,
                    x,
                    y,
                    RenderTexture {
                        bounds: Some(bounds),
                        grayscale,
                        ..Default::default()
                    },
                );
            }
        }
    }
}
