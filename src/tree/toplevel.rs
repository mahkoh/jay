use crate::ifs::wl_seat::SeatId;
use crate::ifs::wl_surface::WlSurface;
use crate::tree::Node;
use crate::utils::linkedlist::LinkedNode;
use crate::utils::numcell::NumCell;
use crate::utils::smallmap::SmallMap;
use std::rc::Rc;

pub trait ToplevelNode {
    fn data(&self) -> &ToplevelData;
    fn parent(&self) -> Option<Rc<dyn Node>>;
    fn as_node(&self) -> &dyn Node;
    fn into_node(self: Rc<Self>) -> Rc<dyn Node>;
    fn accepts_keyboard_focus(&self) -> bool;
    fn default_surface(&self) -> Rc<WlSurface>;
    fn set_active(&self, active: bool);
    fn activate(&self);
    fn toggle_floating(self: Rc<Self>);
}

#[derive(Default)]
pub struct ToplevelData {
    pub active_surfaces: NumCell<u32>,
    pub focus_surface: SmallMap<SeatId, Rc<WlSurface>, 1>,
    pub toplevel_history: SmallMap<SeatId, LinkedNode<Rc<dyn ToplevelNode>>, 1>,
}

impl ToplevelData {
    pub fn clear(&self) {
        self.focus_surface.clear();
        self.toplevel_history.clear();
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
}
