use {
    crate::{
        cursor::Cursor,
        fixed::Fixed,
        ifs::{wl_seat::WlSeatGlobal, wl_surface::WlSurface},
        leaks::Tracker,
        rect::Rect,
        render::Renderer,
        tree::OutputNode,
    },
    std::{cell::Cell, rc::Rc},
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
        let extents = self.extents.get();
        let (hot_x, hot_y) = self.hotspot.get();
        self.extents
            .set(Rect::new_sized(-hot_x, -hot_y, extents.width(), extents.height()).unwrap());
    }

    pub fn handle_surface_destroy(&self) {
        self.seat.set_app_cursor(None);
    }

    pub fn handle_buffer_change(&self) {
        let (width, height) = self.surface.buffer_abs_pos.get().size();
        self.extents
            .set(Rect::new_sized(0, 0, width, height).unwrap());
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
}

impl Cursor for CursorSurface {
    fn render(&self, renderer: &mut Renderer, x: Fixed, y: Fixed) {
        let extents = self.extents.get().move_(x.round_down(), y.round_down());
        if extents.intersects(&renderer.logical_extents()) {
            let scale = renderer.scale();
            if scale != 1 {
                let scale = scale.to_f64();
                let (hot_x, hot_y) = self.hotspot.get();
                let (hot_x, hot_y) = (Fixed::from_int(hot_x), Fixed::from_int(hot_y));
                let x = ((x - hot_x).to_f64() * scale).round() as _;
                let y = ((y - hot_y).to_f64() * scale).round() as _;
                renderer.render_surface_scaled(&self.surface, x, y, None);
            } else {
                renderer.render_surface(&self.surface, extents.x1(), extents.y1());
            }
        }
    }

    fn set_output(&self, output: &Rc<OutputNode>) {
        self.surface.set_output(output);
    }

    fn handle_unset(&self) {
        self.surface.cursors.remove(&self.seat.id());
    }
}
