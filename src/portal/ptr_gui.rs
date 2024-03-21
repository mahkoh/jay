use {
    crate::{
        async_engine::{Phase, SpawnedFuture},
        cursor::KnownCursor,
        fixed::Fixed,
        format::ARGB8888,
        gfx_api::{GfxContext, GfxFramebuffer},
        ifs::zwlr_layer_shell_v1::OVERLAY,
        portal::ptl_display::{PortalDisplay, PortalOutput, PortalSeat},
        renderer::renderer_base::RendererBase,
        scale::Scale,
        text::{self, TextMeasurement, TextTexture},
        theme::Color,
        utils::{
            asyncevent::AsyncEvent, clonecell::CloneCell, copyhashmap::CopyHashMap,
            errorfmt::ErrorFmt, rc_eq::rc_eq,
        },
        video::gbm::GBM_BO_USE_RENDERING,
        wire::{
            wp_fractional_scale_v1::PreferredScale, zwlr_layer_surface_v1::Configure,
            ZwpLinuxBufferParamsV1Id,
        },
        wl_usr::usr_ifs::{
            usr_linux_buffer_params::{UsrLinuxBufferParams, UsrLinuxBufferParamsOwner},
            usr_wl_buffer::{UsrWlBuffer, UsrWlBufferOwner},
            usr_wl_surface::UsrWlSurface,
            usr_wlr_layer_surface::{UsrWlrLayerSurface, UsrWlrLayerSurfaceOwner},
            usr_wp_fractional_scale::{UsrWpFractionalScale, UsrWpFractionalScaleOwner},
            usr_wp_viewport::UsrWpViewport,
        },
    },
    ahash::AHashSet,
    std::{
        borrow::Cow,
        cell::{Cell, RefCell},
        ops::Deref,
        rc::Rc,
    },
};

#[derive(Default)]
pub struct GuiElementData {
    pub x: Cell<f32>,
    pub y: Cell<f32>,
    pub width: Cell<f32>,
    pub height: Cell<f32>,
}

pub trait GuiElement {
    fn data(&self) -> &GuiElementData;
    fn layout(
        &self,
        ctx: &Rc<dyn GfxContext>,
        scale: f32,
        max_width: f32,
        max_height: f32,
    ) -> (f32, f32);
    fn render_at(&self, r: &mut RendererBase, x: f32, y: f32);
    fn child_at(&self, x: f32, y: f32) -> Option<Rc<dyn GuiElement>>;

    fn hover_cursor(&self) -> KnownCursor {
        KnownCursor::Default
    }

    fn button(&self, seat: &PortalSeat, button: u32, state: u32) {
        let _ = seat;
        let _ = button;
        let _ = state;
    }

    fn hover(&self, seat: &PortalSeat, hover: bool) -> bool {
        let _ = seat;
        let _ = hover;
        false
    }

    fn destroy(&self) {}
}

#[derive(Copy, Clone, Debug, Default)]
pub struct ButtonExtents {
    pub width: f32,
    pub height: f32,
    pub tex_off_x: f32,
    pub tex_off_y: f32,
}

pub fn button_extents(
    msmt: &TextMeasurement,
    scale: f32,
    padding: f32,
    border: f32,
) -> ButtonExtents {
    let above_baseline_height =
        (msmt.baseline - msmt.ink_rect.y1().max(msmt.logical_rect.y1())) as f32 / scale;
    let height = above_baseline_height + 2.0 * (padding + border);
    let width = msmt.ink_rect.width() as f32 / scale + 2.0 * (padding + border);
    let tex_off_x = padding + border;
    // let tex_off_y = ((msmt.ink_rect.y1() - msmt.logical_rect.y1()) as f64 / scale).round() as i32 + padding + border;
    let tex_off_y = padding + border;
    ButtonExtents {
        width,
        height,
        tex_off_x,
        tex_off_y,
    }
}

pub struct Button {
    pub data: GuiElementData,
    pub tex_off_x: Cell<f32>,
    pub tex_off_y: Cell<f32>,
    pub hover: RefCell<AHashSet<u32>>,
    pub padding: Cell<f32>,
    pub border: Cell<f32>,
    pub border_color: Cell<Color>,
    pub bg_color: Cell<Color>,
    pub bg_hover_color: Cell<Color>,
    pub text: RefCell<String>,
    pub font: RefCell<Cow<'static, str>>,
    pub tex: CloneCell<Option<TextTexture>>,
    pub owner: CloneCell<Option<Rc<dyn ButtonOwner>>>,
}

pub trait ButtonOwner {
    fn button(&self, button: u32, state: u32);
}

impl Default for Button {
    fn default() -> Self {
        Self {
            data: Default::default(),
            tex_off_x: Cell::new(0.0),
            tex_off_y: Cell::new(0.0),
            hover: Default::default(),
            padding: Default::default(),
            border: Default::default(),
            border_color: Cell::new(Color::from_gray(0)),
            bg_color: Cell::new(Color::from_gray(255)),
            bg_hover_color: Cell::new(Color::from_gray(255)),
            text: Default::default(),
            font: RefCell::new(DEFAULT_FONT.into()),
            tex: Default::default(),
            owner: Default::default(),
        }
    }
}

impl GuiElement for Button {
    fn hover_cursor(&self) -> KnownCursor {
        KnownCursor::Pointer
    }

    fn data(&self) -> &GuiElementData {
        &self.data
    }

    fn layout(
        &self,
        ctx: &Rc<dyn GfxContext>,
        scale: f32,
        _max_width: f32,
        _max_height: f32,
    ) -> (f32, f32) {
        let old_tex = self.tex.take();
        let font = self.font.borrow_mut();
        let text = self.text.borrow_mut();
        let tex = text::render_fitting2(
            ctx,
            old_tex,
            None,
            &font,
            &text,
            Color::from_gray(0),
            false,
            Some(scale as _),
            true,
        )
        .ok();
        let (tex, msmt) = match tex {
            Some((a, b)) => (Some(a), Some(b)),
            _ => (None, None),
        };
        let extents = match msmt {
            Some(m) => button_extents(&m, scale, self.padding.get(), self.border.get()),
            _ => Default::default(),
        };
        self.tex.set(tex);
        self.tex_off_x.set(extents.tex_off_x);
        self.tex_off_y.set(extents.tex_off_y);
        (extents.width, extents.height)
    }

    fn render_at(&self, r: &mut RendererBase, x1: f32, y1: f32) {
        let x2 = x1 + self.data.width.get();
        let y2 = y1 + self.data.height.get();
        let border = self.border.get();
        {
            let rects = [
                (x1, y1, x2, y1 + border),
                (x1, y2 - border, x2, y2),
                (x1, y1 + border, x1 + border, y2 - border),
                (x2 - border, y1 + border, x2, y2 - border),
            ];
            r.fill_boxes_f(&rects, &self.border_color.get());
        }
        {
            let rects = [(x1 + border, y1 + border, x2 - border, y2 - border)];
            let color = match self.hover.borrow_mut().is_empty() {
                true => self.bg_color.get(),
                false => self.bg_hover_color.get(),
            };
            r.fill_boxes_f(&rects, &color);
        }
        if let Some(tex) = self.tex.get() {
            let (tx, ty) = r.scale_point_f(x1 + self.tex_off_x.get(), y1 + self.tex_off_y.get());
            r.render_texture(
                &tex.texture,
                tx.round() as _,
                ty.round() as _,
                None,
                None,
                r.scale(),
                None,
                None,
            );
        }
    }

    fn child_at(&self, _x: f32, _y: f32) -> Option<Rc<dyn GuiElement>> {
        None
    }

    fn hover(&self, seat: &PortalSeat, hover: bool) -> bool {
        let ret;
        let mut set = self.hover.borrow_mut();
        if hover {
            ret = set.is_empty();
            set.insert(seat.global_id);
        } else {
            set.remove(&seat.global_id);
            ret = set.is_empty();
        }
        ret
    }

    fn destroy(&self) {
        self.owner.take();
    }

    fn button(&self, _seat: &PortalSeat, button: u32, state: u32) {
        if let Some(owner) = self.owner.get() {
            owner.button(button, state);
        }
    }
}

const DEFAULT_FONT: &str = "sans-serif 16";

pub struct Label {
    pub data: GuiElementData,
    pub font: RefCell<Cow<'static, str>>,
    pub text: RefCell<String>,
    pub tex: CloneCell<Option<TextTexture>>,
}

impl Default for Label {
    fn default() -> Self {
        Self {
            data: Default::default(),
            font: RefCell::new(DEFAULT_FONT.into()),
            text: RefCell::new("".to_string()),
            tex: Default::default(),
        }
    }
}

impl GuiElement for Label {
    fn data(&self) -> &GuiElementData {
        &self.data
    }

    fn layout(
        &self,
        ctx: &Rc<dyn GfxContext>,
        scale: f32,
        _max_width: f32,
        _max_height: f32,
    ) -> (f32, f32) {
        let old_tex = self.tex.take();
        let text = self.text.borrow_mut();
        let font = self.font.borrow_mut();
        let tex = text::render_fitting2(
            ctx,
            old_tex,
            None,
            &font,
            &text,
            Color::from_gray(255),
            false,
            Some(scale as _),
            false,
        )
        .ok();
        let (tex, width, height) = match tex {
            Some((t, _)) => {
                let (width, height) = t.texture.size();
                (Some(t.clone()), width, height)
            }
            _ => (None, 0, 0),
        };
        self.tex.set(tex);
        (width as f32 / scale, height as f32 / scale)
    }

    fn render_at(&self, r: &mut RendererBase, x: f32, y: f32) {
        if let Some(tex) = self.tex.get() {
            let (tx, ty) = r.scale_point_f(x, y);
            r.render_texture(
                &tex.texture,
                tx.round() as _,
                ty.round() as _,
                None,
                None,
                r.scale(),
                None,
                None,
            );
        }
    }

    fn child_at(&self, _x: f32, _y: f32) -> Option<Rc<dyn GuiElement>> {
        None
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Default)]
pub enum Align {
    #[allow(dead_code)]
    Left,
    #[default]
    Center,
    #[allow(dead_code)]
    Right,
}

#[derive(Copy, Clone, Eq, PartialEq, Default)]
pub enum Orientation {
    #[default]
    Horizontal,
    #[allow(dead_code)]
    Vertical,
}

#[derive(Default)]
pub struct Flow {
    pub data: GuiElementData,
    pub in_margin: Cell<f32>,
    pub cross_margin: Cell<f32>,
    pub orientation: Cell<Orientation>,
    pub cross_align: Cell<Align>,
    pub elements: RefCell<Vec<Rc<dyn GuiElement>>>,
}

impl GuiElement for Flow {
    fn data(&self) -> &GuiElementData {
        &self.data
    }

    fn layout(
        &self,
        ctx: &Rc<dyn GfxContext>,
        scale: f32,
        max_width: f32,
        max_height: f32,
    ) -> (f32, f32) {
        let elements = self.elements.borrow_mut();
        let orientation = self.orientation.get();
        let (max_in_size, _max_cross_size) = match orientation {
            Orientation::Horizontal => (max_width, max_height),
            Orientation::Vertical => (max_height, max_width),
        };
        let mut runs = vec![];
        let mut run = vec![];
        let cross_margin = self.cross_margin.get();
        let in_margin = self.in_margin.get();
        {
            let mut run_cross_size: f32 = 0.0;
            let mut in_pos = in_margin;
            for element in elements.deref() {
                let (w, h) = element.layout(ctx, scale, max_width, max_height);
                let (cur_in_size, cur_cross_size) = match orientation {
                    Orientation::Horizontal => (w, h),
                    Orientation::Vertical => (h, w),
                };
                if in_pos + cur_in_size > max_in_size && run.len() > 0 {
                    runs.push((run, run_cross_size));
                    run = vec![];
                    in_pos = in_margin;
                    run_cross_size = 0.0;
                }
                run_cross_size = run_cross_size.max(cur_cross_size);
                run.push((element, cur_in_size, cur_cross_size));
                in_pos += cur_in_size + in_margin;
            }
            if run.len() > 0 {
                runs.push((run, run_cross_size));
            }
        }
        let mut max_in_pos: f32 = 0.0;
        let mut cross_pos = cross_margin;
        for (run, run_cross_size) in runs {
            let mut in_pos = in_margin;
            for (element, cur_in_size, cur_cross_size) in run {
                let cur_cross_pos = cross_pos
                    + match self.cross_align.get() {
                        Align::Left => 0.0,
                        Align::Center => (run_cross_size - cur_cross_size) / 2.0,
                        Align::Right => run_cross_size - cur_cross_size,
                    };
                let (x, y, w, h) = match orientation {
                    Orientation::Horizontal => (in_pos, cur_cross_pos, cur_in_size, cur_cross_size),
                    Orientation::Vertical => (cur_cross_pos, in_pos, cur_cross_size, cur_in_size),
                };
                element.data().x.set(x);
                element.data().y.set(y);
                element.data().width.set(w);
                element.data().height.set(h);
                in_pos += in_margin + cur_in_size;
            }
            max_in_pos = max_in_pos.max(in_pos);
            cross_pos += cross_margin + run_cross_size;
        }
        let (w, h) = match orientation {
            Orientation::Horizontal => (max_in_pos, cross_pos),
            Orientation::Vertical => (cross_pos, max_in_pos),
        };
        (w.min(max_width), h.min(max_height))
    }

    fn render_at(&self, r: &mut RendererBase, x: f32, y: f32) {
        for element in self.elements.borrow_mut().deref() {
            element.render_at(r, x + element.data().x.get(), y + element.data().y.get());
        }
    }

    fn child_at(&self, x: f32, y: f32) -> Option<Rc<dyn GuiElement>> {
        for child in self.elements.borrow_mut().deref() {
            let data = child.data();
            let x1 = data.x.get();
            let y1 = data.y.get();
            if x >= x1 && x - x1 < data.width.get() && y >= y1 && y - y1 < data.height.get() {
                return Some(child.clone());
            }
        }
        None
    }

    fn destroy(&self) {
        for element in self.elements.borrow_mut().drain(..) {
            element.destroy();
        }
    }
}

pub struct OverlayWindow {
    pub layer_surface: Rc<UsrWlrLayerSurface>,
    pub data: Rc<WindowData>,
    pub owner: CloneCell<Option<Rc<dyn OverlayWindowOwner>>>,
}

pub trait OverlayWindowOwner {
    fn kill(&self, upwards: bool);
}

pub struct WindowData {
    pub frame_missed: Cell<bool>,
    pub first_scale: Cell<bool>,
    pub have_frame: Cell<bool>,
    pub scale: Cell<Scale>,
    pub render_trigger: AsyncEvent,
    pub render_task: Cell<Option<SpawnedFuture<()>>>,
    pub dpy: Rc<PortalDisplay>,
    pub content: CloneCell<Option<Rc<dyn GuiElement>>>,
    pub surface: Rc<UsrWlSurface>,
    pub viewport: Rc<UsrWpViewport>,
    pub fractional_scale: Rc<UsrWpFractionalScale>,
    pub bufs: RefCell<Vec<Rc<GuiBuffer>>>,
    pending_bufs: CopyHashMap<ZwpLinuxBufferParamsV1Id, Rc<GuiBufferPending>>,
    pub width: Cell<i32>,
    pub height: Cell<i32>,
    pub owner: CloneCell<Option<Rc<dyn WindowDataOwner>>>,
    pub seats: CopyHashMap<u32, Rc<GuiWindowSeatState>>,
}

#[derive(Default)]
pub struct GuiWindowSeatState {
    pub x: Cell<f32>,
    pub y: Cell<f32>,
    pub tree: RefCell<Vec<Rc<dyn GuiElement>>>,
    pub cursor: Cell<Option<KnownCursor>>,
}

pub trait WindowDataOwner {
    fn post_layout(&self);
    fn kill(&self, upwards: bool);
}

impl WindowDataOwner for OverlayWindow {
    fn post_layout(&self) {
        self.layer_surface
            .set_size(self.data.width.get(), self.data.height.get());
        self.data.surface.commit();
    }

    fn kill(&self, upwards: bool) {
        if let Some(owner) = self.owner.take() {
            owner.kill(upwards);
        }
        self.layer_surface
            .con
            .remove_obj(self.layer_surface.deref());
    }
}

const NUM_BUFFERS: usize = 2;

impl OverlayWindow {
    pub fn new(output: &Rc<PortalOutput>) -> Rc<Self> {
        let data = WindowData::new(&output.dpy);
        let layer_surface = output
            .dpy
            .ls
            .get_layer_surface(&data.surface, &output.wl, OVERLAY);
        layer_surface.set_size(1, 1);
        let slf = Rc::new(Self {
            layer_surface,
            data,
            owner: Default::default(),
        });
        slf.data.owner.set(Some(slf.clone()));
        slf.layer_surface.owner.set(Some(slf.clone()));
        slf.data.surface.commit();
        slf
    }
}

impl WindowData {
    pub fn schedule_render(&self) {
        self.render_trigger.trigger();
    }

    pub fn new(dpy: &Rc<PortalDisplay>) -> Rc<Self> {
        let surface = dpy.comp.create_surface();
        let viewport = dpy.vp.get_viewport(&surface);
        let fractional_scale = dpy.fsm.get_fractional_scale(&surface);
        viewport.set_destination(1, 1);
        let data = Rc::new(WindowData {
            frame_missed: Cell::new(false),
            first_scale: Cell::new(true),
            have_frame: Cell::new(true),
            bufs: Default::default(),
            pending_bufs: Default::default(),
            width: Cell::new(0),
            height: Cell::new(0),
            owner: Default::default(),
            render_trigger: Default::default(),
            render_task: Cell::new(None),
            dpy: dpy.clone(),
            content: Default::default(),
            surface,
            viewport,
            scale: Cell::new(Scale::from_int(1)),
            fractional_scale,
            seats: Default::default(),
        });
        data.render_task.set(Some(
            dpy.state
                .eng
                .spawn2(Phase::Present, data.clone().render_task()),
        ));
        data.fractional_scale.owner.set(Some(data.clone()));
        data
    }

    pub fn layout(&self) {
        let ctx = match self.dpy.render_ctx.get() {
            Some(ctx) => ctx,
            _ => return,
        };
        let scale = self.scale.get().to_f64() as f32;
        let content = match self.content.get() {
            Some(c) => c,
            _ => return,
        };
        let (mut width, mut height) = content.layout(&ctx.ctx, scale, f32::INFINITY, f32::INFINITY);
        content.data().width.set(width);
        content.data().height.set(height);
        width = width.max(1.0);
        height = height.max(1.0);
        self.width.set(width.round() as _);
        self.height.set(height.round() as _);
        self.viewport
            .set_destination(width.round() as _, height.round() as _);
        if let Some(owner) = self.owner.get() {
            owner.post_layout();
        }
    }

    async fn render_task(self: Rc<Self>) {
        loop {
            self.render_trigger.triggered().await;
            self.render();
        }
    }

    fn render(self: &Rc<Self>) {
        self.frame_missed.set(true);
        if !self.have_frame.get() {
            return;
        }
        let bufs = self.bufs.borrow_mut();
        let buf = 'get_buf: {
            for buf in bufs.deref() {
                if buf.free.get() {
                    break 'get_buf buf;
                }
            }
            return;
        };
        self.frame_missed.set(false);

        self.surface.frame({
            let slf = self.clone();
            move || {
                slf.have_frame.set(true);
                if slf.frame_missed.get() {
                    slf.schedule_render();
                }
            }
        });

        self.have_frame.set(false);
        buf.free.set(false);

        buf.fb
            .render_custom(self.scale.get(), Some(&Color::from_gray(0)), &mut |r| {
                if let Some(content) = self.content.get() {
                    content.render_at(r, 0.0, 0.0)
                }
            });

        self.surface.attach(&buf.wl);
        self.surface.commit();
    }

    pub fn kill(&self, upwards: bool) {
        if let Some(owner) = self.owner.take() {
            owner.kill(upwards);
        }
        self.render_task.take();
        for (_, pb) in self.pending_bufs.lock().drain() {
            pb.params.con.remove_obj(pb.params.deref());
        }
        for buf in self.bufs.borrow_mut().drain(..) {
            buf.wl.con.remove_obj(buf.wl.deref());
        }
        self.fractional_scale
            .con
            .remove_obj(self.fractional_scale.deref());
        self.viewport.con.remove_obj(self.viewport.deref());
        self.surface.con.remove_obj(self.surface.deref());
        if let Some(content) = self.content.take() {
            content.destroy();
        }
    }

    pub fn allocate_buffers(self: &Rc<Self>) {
        {
            for (_, buf) in self.pending_bufs.lock().drain() {
                buf.params.con.remove_obj(buf.params.deref());
            }
        }
        {
            let mut bufs = self.bufs.borrow_mut();
            for buf in bufs.drain(..) {
                buf.wl.con.remove_obj(buf.wl.deref());
            }
        }
        let ctx = match self.dpy.render_ctx.get() {
            Some(ctx) => ctx,
            _ => return,
        };
        let dmabuf = match self.dpy.dmabuf.get() {
            Some(dmabuf) => dmabuf,
            _ => return,
        };
        self.frame_missed.set(true);
        let width = (self.width.get() as f64 * self.scale.get().to_f64()).round() as i32;
        let height = (self.height.get() as f64 * self.scale.get().to_f64()).round() as i32;
        let formats = ctx.ctx.formats();
        let format = match formats.get(&ARGB8888.drm) {
            None => {
                log::error!("Render context does not support ARGB8888 format");
                return;
            }
            Some(f) => f,
        };
        if format.write_modifiers.is_empty() {
            log::error!("Render context cannot render to ARGB8888 format");
            return;
        }
        for _ in 0..NUM_BUFFERS {
            let bo = match ctx.ctx.gbm().create_bo(
                &self.dpy.state.dma_buf_ids,
                width,
                height,
                ARGB8888,
                &format.write_modifiers,
                GBM_BO_USE_RENDERING,
            ) {
                Ok(b) => b,
                Err(e) => {
                    log::error!("Could not allocate dmabuf: {}", ErrorFmt(e));
                    return;
                }
            };
            let img = match ctx.ctx.clone().dmabuf_img(bo.dmabuf()) {
                Ok(b) => b,
                Err(e) => {
                    log::error!("Could not import dmabuf into EGL: {}", ErrorFmt(e));
                    return;
                }
            };
            let fb = match img.to_framebuffer() {
                Ok(b) => b,
                Err(e) => {
                    log::error!(
                        "Could not turns EGL image into framebuffer: {}",
                        ErrorFmt(e)
                    );
                    return;
                }
            };
            let params = dmabuf.create_params();
            params.create(bo.dmabuf());
            let pending = Rc::new(GuiBufferPending {
                window: self.clone(),
                fb,
                params,
                size: (width, height),
            });
            pending.params.owner.set(Some(pending.clone()));
            self.pending_bufs.set(pending.params.id, pending.clone());
        }
    }

    fn tree_at(&self, tree: &mut Vec<Rc<dyn GuiElement>>, mut x: f32, mut y: f32) {
        let mut element = match self.content.get() {
            Some(e) => e,
            _ => return,
        };
        tree.push(element.clone());
        while let Some(c) = element.child_at(x, y) {
            tree.push(c.clone());
            x -= c.data().x.get();
            y -= c.data().y.get();
            element = c;
        }
    }

    pub fn motion(&self, pseat: &PortalSeat, x: Fixed, y: Fixed, _enter: bool) {
        let x = x.to_f64() as f32;
        let y = y.to_f64() as f32;
        let seat = self
            .seats
            .lock()
            .entry(pseat.global_id)
            .or_default()
            .clone();
        seat.x.set(x);
        seat.y.set(y);

        let mut tree = seat.tree.borrow_mut();
        let old_element = tree.last().cloned();
        self.tree_at(&mut tree, x, y);
        let new_element = tree.last().cloned();

        let element_changed = match (&old_element, &new_element) {
            (Some(old), Some(new)) => !rc_eq(old, new),
            (None, None) => false,
            _ => true,
        };

        if element_changed {
            if let Some(o) = &old_element {
                o.hover(pseat, false);
            }
            if let Some(o) = &new_element {
                o.hover(pseat, true);
            }
        }

        if element_changed {
            self.schedule_render();
        }

        let cursor = match &new_element {
            Some(e) => e.hover_cursor(),
            _ => KnownCursor::Default,
        };

        if seat.cursor.replace(Some(cursor)) != Some(cursor) {
            pseat.jay_pointer.set_known_cursor(cursor);
        }
    }

    pub fn button(&self, pseat: &PortalSeat, button: u32, state: u32) {
        let seat = match self.seats.get(&pseat.global_id) {
            Some(s) => s,
            _ => return,
        };
        let element = seat.tree.borrow_mut().last().cloned();
        if let Some(e) = element {
            e.button(pseat, button, state);
        }
    }
}

pub struct GuiBuffer {
    pub wl: Rc<UsrWlBuffer>,
    pub window: Rc<WindowData>,
    pub fb: Rc<dyn GfxFramebuffer>,
    pub free: Cell<bool>,
    pub size: (i32, i32),
}

struct GuiBufferPending {
    pub window: Rc<WindowData>,
    pub fb: Rc<dyn GfxFramebuffer>,
    pub params: Rc<UsrLinuxBufferParams>,
    pub size: (i32, i32),
}

impl UsrWlBufferOwner for GuiBuffer {
    fn release(&self) {
        self.free.set(true);
        if self.window.frame_missed.get() {
            self.window.schedule_render();
        }
    }
}

impl UsrWpFractionalScaleOwner for WindowData {
    fn preferred_scale(self: Rc<Self>, ev: &PreferredScale) {
        let mut layout = self.first_scale.replace(false);
        let scale = Scale::from_wl(ev.scale);
        layout |= self.scale.replace(scale) != scale;
        if layout {
            self.layout();
            self.allocate_buffers();
        }
    }
}

impl UsrWlrLayerSurfaceOwner for OverlayWindow {
    fn configure(&self, _ev: &Configure) {
        self.data.schedule_render();
    }

    fn closed(&self) {
        self.data.kill(true);
    }
}

impl UsrLinuxBufferParamsOwner for GuiBufferPending {
    fn created(&self, buffer: Rc<UsrWlBuffer>) {
        buffer.con.add_object(buffer.clone());
        let buf = Rc::new(GuiBuffer {
            wl: buffer,
            window: self.window.clone(),
            fb: self.fb.clone(),
            free: Cell::new(true),
            size: self.size,
        });
        buf.wl.owner.set(Some(buf.clone()));
        self.window.bufs.borrow_mut().push(buf);
        self.params.con.remove_obj(self.params.deref());
        self.window.pending_bufs.remove(&self.params.id);
        if self.window.frame_missed.get() {
            self.window.schedule_render();
        }
    }

    fn failed(&self) {
        self.window.kill(true);
    }
}
