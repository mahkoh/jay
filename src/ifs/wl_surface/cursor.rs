use {
    crate::{
        cursor::Cursor,
        fixed::Fixed,
        ifs::{wl_seat::WlSeatGlobal, wl_surface::WlSurface},
        leaks::Tracker,
        rect::Rect,
        renderer::Renderer,
        scale::Scale,
        tree::{Node, NodeVisitorBase, OutputNode},
    },
    std::{cell::Cell, ops::Deref, rc::Rc},
};

pub struct CursorSurface {
    seat: Rc<WlSeatGlobal>,
    surface: Rc<WlSurface>,
    hotspot: Cell<(i32, i32)>,
    extents: Cell<Rect>,
    pub tracker: Tracker<Self>,
}

impl CursorSurface {
    pub fn new(seat: &Rc<WlSeatGlobal>, surface: &Rc<WlSurface>) -> Self {
        Self {
            seat: seat.clone(),
            surface: surface.clone(),
            hotspot: Cell::new((0, 0)),
            extents: Cell::new(Default::default()),
            tracker: Default::default(),
        }
    }

    fn update_extents(&self) {
        let (hot_x, hot_y) = self.hotspot.get();
        self.extents
            .set(self.surface.extents.get().move_(-hot_x, -hot_y));
    }

    pub fn handle_surface_destroy(&self) {
        self.seat.set_app_cursor(None);
    }

    pub fn handle_buffer_change(&self) {
        self.update_extents();
    }

    pub fn set_hotspot(&self, x: i32, y: i32) {
        self.hotspot.set((x, y));
        self.update_extents();
    }

    pub fn dec_hotspot(&self, hotspot_dx: i32, hotspot_dy: i32) {
        let (hot_x, hot_y) = self.hotspot.get();
        self.hotspot.set((hot_x - hotspot_dx, hot_y - hotspot_dy));
        self.update_extents();
    }

    pub fn update_hardware_cursor(&self) {
        if self.seat.hardware_cursor() {
            self.seat.update_hardware_cursor();
        }
    }
}

impl Cursor for CursorSurface {
    fn render(&self, renderer: &mut Renderer, x: Fixed, y: Fixed) {
        let x_int = x.round_down();
        let y_int = y.round_down();
        let extents = self.extents.get().move_(x_int, y_int);
        if extents.intersects(&renderer.logical_extents()) {
            let (hot_x, hot_y) = self.hotspot.get();
            let scale = renderer.scale();
            if scale != 1 {
                let scale = scale.to_f64();
                let (hot_x, hot_y) = (Fixed::from_int(hot_x), Fixed::from_int(hot_y));
                let x = ((x - hot_x).to_f64() * scale).round() as _;
                let y = ((y - hot_y).to_f64() * scale).round() as _;
                renderer.render_surface_scaled(&self.surface, x, y, None, None, false);
            } else {
                renderer.render_surface(&self.surface, x_int - hot_x, y_int - hot_y, None);
            }
        }
    }

    fn render_hardware_cursor(&self, renderer: &mut Renderer) {
        let extents = self.surface.extents.get();
        renderer.render_surface(&self.surface, -extents.x1(), -extents.y1(), None);

        struct FrameRequests;
        impl NodeVisitorBase for FrameRequests {
            fn visit_surface(&mut self, node: &Rc<WlSurface>) {
                for fr in node.frame_requests.borrow_mut().drain(..) {
                    fr.send_done();
                    let _ = fr.client.remove_obj(fr.deref());
                }
                for fr in node.presentation_feedback.borrow_mut().drain(..) {
                    fr.send_discarded();
                    let _ = fr.client.remove_obj(fr.deref());
                }
                node.node_visit_children(self);
            }
        }
        FrameRequests.visit_surface(&self.surface);
    }

    fn extents_at_scale(&self, scale: Scale) -> Rect {
        let rect = self.extents.get();
        if scale == 1 {
            return rect;
        }
        let scale = scale.to_f64();
        Rect::new(
            (rect.x1() as f64 * scale).ceil() as _,
            (rect.y1() as f64 * scale).ceil() as _,
            (rect.x2() as f64 * scale).ceil() as _,
            (rect.y2() as f64 * scale).ceil() as _,
        )
        .unwrap()
    }

    fn set_output(&self, output: &Rc<OutputNode>) {
        self.surface.set_output(output);
    }

    fn handle_set(self: Rc<Self>) {
        self.surface.cursors.insert(self.seat.id(), self.clone());
        if self.surface.cursors.is_not_empty() {
            self.surface
                .set_visible(self.surface.client.state.root_visible());
        }
    }

    fn handle_unset(&self) {
        self.surface.cursors.remove(&self.seat.id());
        if self.surface.cursors.is_empty() {
            self.surface.set_visible(false);
        }
    }

    fn set_visible(&self, visible: bool) {
        self.surface.set_visible(visible);
    }
}
