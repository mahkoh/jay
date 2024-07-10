use {
    crate::{
        ifs::{wl_seat::WlSeatGlobal, wl_surface::WlSurface},
        rect::Rect,
        renderer::Renderer,
    },
    std::rc::Rc,
};

pub struct DndIcon {
    pub(super) surface: Rc<WlSurface>,
    pub(super) seat: Rc<WlSeatGlobal>,
}

impl DndIcon {
    pub fn surface(&self) -> &Rc<WlSurface> {
        &self.surface
    }

    fn update_visible(&self) {
        let is_visible =
            self.surface.dnd_icons.is_not_empty() && self.surface.client.state.root_visible();
        self.surface.set_visible(is_visible);
    }

    pub fn enable(self: &Rc<Self>) {
        self.surface.dnd_icons.insert(self.seat.id(), self.clone());
        self.update_visible();
    }

    pub fn disable(self: &Rc<Self>) {
        self.surface.dnd_icons.remove(&self.seat.id());
        self.update_visible();
    }

    pub fn surface_position(&self, seat_x: i32, seat_y: i32) -> (i32, i32) {
        (
            seat_x + self.surface.buf_x.get(),
            seat_y + self.surface.buf_y.get(),
        )
    }

    fn extents(&self, x: i32, y: i32) -> Rect {
        let (x, y) = self.surface_position(x, y);
        self.surface.extents.get().move_(x, y)
    }

    pub fn render(&self, renderer: &mut Renderer<'_>, cursor_rect: &Rect, x: i32, y: i32) {
        let extents = self.extents(x, y);
        if extents.intersects(&cursor_rect) {
            let (x, y) = cursor_rect.translate(extents.x1(), extents.y1());
            renderer.render_surface(&self.surface, x, y, None);
        }
    }
}
