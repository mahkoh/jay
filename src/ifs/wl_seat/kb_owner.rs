use {
    crate::{ifs::wl_seat::WlSeatGlobal, tree::Node, utils::clonecell::CloneCell},
    std::rc::Rc,
};
use crate::xwayland::XWaylandEvent;

pub struct KbOwnerHolder {
    default: Rc<DefaultKbOwner>,
    owner: CloneCell<Rc<dyn KbOwner>>,
}

impl Default for KbOwnerHolder {
    fn default() -> Self {
        Self {
            default: Rc::new(DefaultKbOwner),
            owner: CloneCell::new(Rc::new(DefaultKbOwner)),
        }
    }
}

impl KbOwnerHolder {
    pub fn grab(&self, seat: &Rc<WlSeatGlobal>, node: Rc<dyn Node>) -> bool {
        self.owner.get().grab(seat, node)
    }

    pub fn ungrab(&self, seat: &Rc<WlSeatGlobal>) {
        self.owner.get().ungrab(seat)
    }

    pub fn set_kb_node(&self, seat: &Rc<WlSeatGlobal>, node: Rc<dyn Node>) {
        self.owner.get().set_kb_node(seat, node);
    }
}

struct DefaultKbOwner;

struct GrabKbOwner;

trait KbOwner {
    fn grab(&self, seat: &Rc<WlSeatGlobal>, node: Rc<dyn Node>) -> bool;
    fn ungrab(&self, seat: &Rc<WlSeatGlobal>);
    fn set_kb_node(&self, seat: &Rc<WlSeatGlobal>, node: Rc<dyn Node>);
}

impl KbOwner for DefaultKbOwner {
    fn grab(&self, seat: &Rc<WlSeatGlobal>, node: Rc<dyn Node>) -> bool {
        self.set_kb_node(seat, node);
        seat.kb_owner.owner.set(Rc::new(GrabKbOwner));
        true
    }

    fn ungrab(&self, _seat: &Rc<WlSeatGlobal>) {
        // nothing
    }

    fn set_kb_node(&self, seat: &Rc<WlSeatGlobal>, node: Rc<dyn Node>) {
        let old = seat.keyboard_node.get();
        if old.node_id() == node.node_id() {
            return;
        }
        log::info!("unfocus {}", old.node_id());
        if old.node_is_xwayland_surface() && !node.node_is_xwayland_surface() {
            seat.state.xwayland.queue.push(XWaylandEvent::ActivateRoot);
        }
        old.node_unfocus(seat);
        if old.node_seat_state().unfocus(seat) {
            old.node_active_changed(false);
        }

        if node.node_seat_state().focus(seat) {
            node.node_active_changed(true);
        }
        log::info!("focus {}", node.node_id());
        node.clone().node_focus(seat);
        seat.keyboard_node.set(node.clone());
    }
}

impl KbOwner for GrabKbOwner {
    fn grab(&self, _seat: &Rc<WlSeatGlobal>, _node: Rc<dyn Node>) -> bool {
        false
    }

    fn ungrab(&self, seat: &Rc<WlSeatGlobal>) {
        seat.kb_owner.owner.set(seat.kb_owner.default.clone());
    }

    fn set_kb_node(&self, _seat: &Rc<WlSeatGlobal>, _node: Rc<dyn Node>) {
        // nothing
    }
}
