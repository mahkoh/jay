use {
    crate::{
        ifs::{wl_seat::SeatId, wl_surface::WlSurface},
        tree::{Node, WorkspaceNode},
        utils::{numcell::NumCell, smallmap::SmallMap},
    },
    std::{cell::Cell, rc::Rc},
};

tree_id!(ToplevelNodeId);
pub trait ToplevelNode {
    fn data(&self) -> &ToplevelData;
    fn as_node(&self) -> &dyn Node;
    fn into_node(self: Rc<Self>) -> Rc<dyn Node>;
    fn accepts_keyboard_focus(&self) -> bool;
    fn default_surface(&self) -> Rc<WlSurface>;
    fn set_active(&self, active: bool);
    fn activate(&self);
    fn toggle_floating(self: Rc<Self>);
    fn close(&self);
}

#[derive(Default)]
pub struct ToplevelData {
    pub active_surfaces: NumCell<u32>,
    pub focus_surface: SmallMap<SeatId, Rc<WlSurface>, 1>,
    pub is_floating: Cell<bool>,
    pub float_width: Cell<i32>,
    pub float_height: Cell<i32>,
}

impl ToplevelData {
    pub fn clear(&self) {
        self.focus_surface.clear();
    }

    pub fn float_size(&self, ws: &WorkspaceNode) -> (i32, i32) {
        let output = ws.output.get().global.pos.get();
        let mut width = self.float_width.get();
        let mut height = self.float_height.get();
        if width == 0 {
            width = output.width() / 2;
        }
        if height == 0 {
            height = output.height() / 2;
        }
        (width, height)
    }
}

impl<'a> dyn ToplevelNode + 'a {
    pub fn surface_active_changed(&self, active: bool) {
        if active {
            if self.data().active_surfaces.fetch_add(1) == 0 {
                self.set_active(true);
            }
        } else {
            if self.data().active_surfaces.fetch_sub(1) == 1 {
                self.set_active(false);
            }
        }
    }

    pub fn focus_surface(&self, seat: SeatId) -> Rc<WlSurface> {
        self.data()
            .focus_surface
            .get(&seat)
            .unwrap_or_else(|| self.default_surface())
    }

    pub fn parent(&self) -> Option<Rc<dyn Node>> {
        self.as_node().node_parent()
    }
}
