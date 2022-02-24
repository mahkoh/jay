use crate::ifs::wl_seat::WlSeatGlobal;
use crate::ifs::wl_surface::WlSurface;
use crate::tree::Node;
use crate::utils::linkedlist::LinkedNode;
use std::rc::Rc;

pub trait ToplevelNode: Node {
    fn parent(&self) -> Option<Rc<dyn Node>>;
    fn focus_surface(&self, seat: &WlSeatGlobal) -> Rc<WlSurface>;
    fn set_focus_history_link(&self, seat: &WlSeatGlobal, link: LinkedNode<Rc<dyn ToplevelNode>>);
    fn as_node(&self) -> &dyn Node;

    fn parent_is_float(&self) -> bool {
        if let Some(parent) = self.parent() {
            return parent.is_float();
        }
        false
    }
}
