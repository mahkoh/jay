use {
    crate::{
        cursor::Cursor,
        ifs::{wl_seat::WlSeatGlobal, wl_surface::WlSurface},
        leaks::Tracker,
        rect::Rect,
        render::Renderer,
    },
    std::{cell::Cell, rc::Rc},
};

pub struct CursorSurface {
    seat: Rc<WlSeatGlobal>,
    surface: Rc<WlSurface>,
    hotspot: Cell<(i32, i32)>,
    pos: Cell<(i32, i32)>,
    extents: Cell<Rect>,
    pub tracker: Tracker<Self>,
}

impl CursorSurface {
    pub fn new(seat: &Rc<WlSeatGlobal>, surface: &Rc<WlSurface>) -> Self {
        Self {
            seat: seat.clone(),
            surface: surface.clone(),
            hotspot: Cell::new((0, 0)),
            pos: Cell::new((0, 0)),
            extents: Cell::new(Default::default()),
            tracker: Default::default(),
        }
    }

    fn update_extents(&self) {
        let (pos_x, pos_y) = self.pos.get();
        let extents = self.extents.get();
        let (hot_x, hot_y) = self.hotspot.get();
        self.extents.set(
            Rect::new_sized(
                pos_x - hot_x,
                pos_y - hot_y,
                extents.width(),
                extents.height(),
            )
            .unwrap(),
        );
    }

    pub fn handle_surface_destroy(&self) {
        self.seat.set_cursor(None);
    }

    pub fn handle_buffer_change(&self) {
        let (width, height) = match self.surface.buffer.get() {
            Some(b) => (b.rect.width(), b.rect.height()),
            _ => (0, 0),
        };
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
    fn set_position(&self, x: i32, y: i32) {
        self.pos.set((x, y));
        self.update_extents();
    }

    fn render(&self, renderer: &mut Renderer, x: i32, y: i32) {
        renderer.render_surface(&self.surface, x, y);
    }

    fn get_hotspot(&self) -> (i32, i32) {
        self.hotspot.get()
    }

    fn extents(&self) -> Rect {
        self.extents.get()
    }

    fn handle_unset(&self) {
        self.surface.cursors.remove(&self.seat.id());
    }
}
