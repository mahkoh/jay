use crate::ifs::wl_seat::WlSeatGlobal;
use crate::tree::Node;
use crate::CloneCell;
use std::rc::Rc;

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
        if old.id() == node.id() {
            return;
        }
        old.unfocus(seat);
        if old.seat_state().unfocus(seat) {
            old.active_changed(false);
        }

        if node.seat_state().focus(seat) {
            node.active_changed(true);
        }
        node.clone().focus(seat);
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
