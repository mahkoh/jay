use {
    crate::{fixed::Fixed, ifs::wl_seat::WlSeatGlobal, tree::Node, utils::clonecell::CloneCell},
    std::rc::Rc,
};

pub struct GestureOwnerHolder {
    default: Rc<NoGesture>,
    owner: CloneCell<Rc<dyn GestureOwner>>,
}

impl Default for GestureOwnerHolder {
    fn default() -> Self {
        let default = Rc::new(NoGesture);
        Self {
            owner: CloneCell::new(default.clone()),
            default,
        }
    }
}

impl GestureOwnerHolder {
    pub fn revert_to_default(&self, seat: &Rc<WlSeatGlobal>) {
        self.owner.get().revert_to_default(seat);
        self.set_default_owner();
    }

    pub fn swipe_begin(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, finger_count: u32) {
        self.owner.get().swipe_begin(seat, time_usec, finger_count)
    }

    pub fn swipe_update(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, dx: Fixed, dy: Fixed) {
        self.owner.get().swipe_update(seat, time_usec, dx, dy)
    }

    pub fn swipe_end(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, cancelled: bool) {
        self.owner.get().swipe_end(seat, time_usec, cancelled)
    }

    pub fn pinch_begin(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, finger_count: u32) {
        self.owner.get().pinch_begin(seat, time_usec, finger_count)
    }

    pub fn pinch_update(
        &self,
        seat: &Rc<WlSeatGlobal>,
        time_usec: u64,
        dx: Fixed,
        dy: Fixed,
        scale: Fixed,
        rotation: Fixed,
    ) {
        self.owner
            .get()
            .pinch_update(seat, time_usec, dx, dy, scale, rotation)
    }

    pub fn pinch_end(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, cancelled: bool) {
        self.owner.get().pinch_end(seat, time_usec, cancelled)
    }

    pub fn hold_begin(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, finger_count: u32) {
        self.owner.get().hold_begin(seat, time_usec, finger_count)
    }

    pub fn hold_end(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, cancelled: bool) {
        self.owner.get().hold_end(seat, time_usec, cancelled)
    }

    fn set_default_owner(&self) {
        self.owner.set(self.default.clone());
    }
}

trait GestureOwner {
    fn revert_to_default(&self, seat: &Rc<WlSeatGlobal>);

    fn swipe_begin(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, finger_count: u32) {
        let _ = seat;
        let _ = time_usec;
        let _ = finger_count;
    }

    fn swipe_update(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, dx: Fixed, dy: Fixed) {
        let _ = seat;
        let _ = time_usec;
        let _ = dx;
        let _ = dy;
    }

    fn swipe_end(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, cancelled: bool) {
        let _ = seat;
        let _ = time_usec;
        let _ = cancelled;
    }

    fn pinch_begin(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, finger_count: u32) {
        let _ = seat;
        let _ = time_usec;
        let _ = finger_count;
    }

    fn pinch_update(
        &self,
        seat: &Rc<WlSeatGlobal>,
        time_usec: u64,
        dx: Fixed,
        dy: Fixed,
        scale: Fixed,
        rotation: Fixed,
    ) {
        let _ = seat;
        let _ = time_usec;
        let _ = dx;
        let _ = dy;
        let _ = scale;
        let _ = rotation;
    }

    fn pinch_end(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, cancelled: bool) {
        let _ = seat;
        let _ = time_usec;
        let _ = cancelled;
    }

    fn hold_begin(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, finger_count: u32) {
        let _ = seat;
        let _ = time_usec;
        let _ = finger_count;
    }

    fn hold_end(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, cancelled: bool) {
        let _ = seat;
        let _ = time_usec;
        let _ = cancelled;
    }
}

struct NoGesture;

impl GestureOwner for NoGesture {
    fn revert_to_default(&self, seat: &Rc<WlSeatGlobal>) {
        let _ = seat;
    }

    fn swipe_begin(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, finger_count: u32) {
        let Some(node) = seat.pointer_node() else {
            return;
        };
        node.node_seat_state().gesture_begin(seat);
        node.node_on_swipe_begin(seat, time_usec, finger_count);
        seat.gesture_owner.owner.set(Rc::new(SwipeGesture { node }));
    }

    fn pinch_begin(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, finger_count: u32) {
        let Some(node) = seat.pointer_node() else {
            return;
        };
        node.node_seat_state().gesture_begin(seat);
        node.node_on_pinch_begin(seat, time_usec, finger_count);
        seat.gesture_owner.owner.set(Rc::new(PinchGesture { node }));
    }

    fn hold_begin(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, finger_count: u32) {
        let Some(node) = seat.pointer_node() else {
            return;
        };
        node.node_seat_state().gesture_begin(seat);
        node.node_on_hold_begin(seat, time_usec, finger_count);
        seat.gesture_owner.owner.set(Rc::new(HoldGesture { node }));
    }
}

struct SwipeGesture {
    node: Rc<dyn Node>,
}

impl GestureOwner for SwipeGesture {
    fn revert_to_default(&self, seat: &Rc<WlSeatGlobal>) {
        self.swipe_end(seat, seat.state.now_usec(), true);
    }

    fn swipe_update(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, dx: Fixed, dy: Fixed) {
        self.node.node_on_swipe_update(seat, time_usec, dx, dy);
    }

    fn swipe_end(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, cancelled: bool) {
        self.node.node_on_swipe_end(seat, time_usec, cancelled);
        self.node.node_seat_state().gesture_end(seat);
        seat.gesture_owner.set_default_owner();
    }
}

struct PinchGesture {
    node: Rc<dyn Node>,
}

impl GestureOwner for PinchGesture {
    fn revert_to_default(&self, seat: &Rc<WlSeatGlobal>) {
        self.pinch_end(seat, seat.state.now_usec(), true);
    }

    fn pinch_update(
        &self,
        seat: &Rc<WlSeatGlobal>,
        time_usec: u64,
        dx: Fixed,
        dy: Fixed,
        scale: Fixed,
        rotation: Fixed,
    ) {
        self.node
            .node_on_pinch_update(seat, time_usec, dx, dy, scale, rotation)
    }

    fn pinch_end(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, cancelled: bool) {
        self.node.node_on_pinch_end(seat, time_usec, cancelled);
        self.node.node_seat_state().gesture_end(seat);
        seat.gesture_owner.set_default_owner();
    }
}

struct HoldGesture {
    node: Rc<dyn Node>,
}

impl GestureOwner for HoldGesture {
    fn revert_to_default(&self, seat: &Rc<WlSeatGlobal>) {
        self.hold_end(seat, seat.state.now_usec(), true);
    }

    fn hold_end(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, cancelled: bool) {
        self.node.node_on_hold_end(seat, time_usec, cancelled);
        self.node.node_seat_state().gesture_end(seat);
        seat.gesture_owner.set_default_owner();
    }
}
