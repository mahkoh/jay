use {
    crate::{
        fixed::Fixed,
        ifs::wl_seat::WlSeatGlobal,
        tree::Node,
        utils::{clonecell::CloneCell, smallmap::SmallMap},
    },
    std::rc::Rc,
};

pub struct TouchOwnerHolder {
    default: Rc<DefaultTouchOwner>,
    owner: CloneCell<Rc<dyn TouchOwner>>,
}

impl Default for TouchOwnerHolder {
    fn default() -> Self {
        Self {
            default: Rc::new(DefaultTouchOwner),
            owner: CloneCell::new(Rc::new(DefaultTouchOwner)),
        }
    }
}

impl TouchOwnerHolder {
    pub fn down(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, id: i32, x: Fixed, y: Fixed) {
        self.owner.get().down(seat, time_usec, id, x, y)
    }

    pub fn up(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, id: i32) {
        self.owner.get().up(seat, time_usec, id)
    }

    pub fn motion(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, id: i32, x: Fixed, y: Fixed) {
        self.owner.get().motion(seat, time_usec, id, x, y)
    }

    pub fn frame(&self, seat: &Rc<WlSeatGlobal>) {
        self.owner.get().frame(seat)
    }

    pub fn cancel(&self, seat: &Rc<WlSeatGlobal>) {
        self.owner.get().cancel(seat)
    }

    pub fn clear(&self) {
        self.set_default_owner();
    }

    fn set_default_owner(&self) {
        self.owner.set(self.default.clone());
    }
}

struct DefaultTouchOwner;

struct GrabTouchOwner {
    node: Rc<dyn Node>,
    down_ids: SmallMap<i32, (), 10>,
}

trait TouchOwner {
    fn down(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, id: i32, x: Fixed, y: Fixed);
    fn up(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, id: i32);
    fn motion(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, id: i32, x: Fixed, y: Fixed);
    fn frame(&self, seat: &Rc<WlSeatGlobal>);
    fn cancel(&self, seat: &Rc<WlSeatGlobal>);
}

impl TouchOwner for DefaultTouchOwner {
    fn down(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, id: i32, x: Fixed, y: Fixed) {
        let node = seat.state.node_at(x.round_down(), y.round_down());
        node.node.node_seat_state().touch_begin(seat);
        node.node.node_restack();
        let owner = Rc::new(GrabTouchOwner {
            node: node.node,
            down_ids: Default::default(),
        });
        seat.touch_owner.owner.set(owner.clone());
        owner.down(seat, time_usec, id, x, y);
    }

    fn up(&self, _seat: &Rc<WlSeatGlobal>, _time_usec: u64, _id: i32) {
        // nothing
    }

    fn motion(&self, _seat: &Rc<WlSeatGlobal>, _time_usec: u64, _id: i32, _x: Fixed, _y: Fixed) {
        // nothing
    }

    fn frame(&self, _seat: &Rc<WlSeatGlobal>) {
        // nothing
    }

    fn cancel(&self, _seat: &Rc<WlSeatGlobal>) {
        // nothing
    }
}

impl GrabTouchOwner {
    fn translate(&self, x: Fixed, y: Fixed) -> (Fixed, Fixed) {
        let x_int = x.round_down();
        let y_int = y.round_down();
        let (x_int, y_int) = self.node.node_absolute_position().translate(x_int, y_int);
        (x.apply_fract(x_int), y.apply_fract(y_int))
    }

    fn revert_to_default(&self, seat: &Rc<WlSeatGlobal>) {
        self.node.node_seat_state().touch_end(seat);
        seat.touch_owner.set_default_owner();
    }
}

impl TouchOwner for GrabTouchOwner {
    fn down(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, id: i32, x: Fixed, y: Fixed) {
        if self.down_ids.insert(id, ()).is_some() {
            return;
        }
        let (x, y) = self.translate(x, y);
        self.node
            .clone()
            .node_on_touch_down(seat, time_usec, id, x, y);
    }

    fn up(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, id: i32) {
        if self.down_ids.remove(&id).is_none() {
            return;
        }
        self.node.clone().node_on_touch_up(seat, time_usec, id);
    }

    fn motion(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, id: i32, x: Fixed, y: Fixed) {
        if !self.down_ids.contains(&id) {
            return;
        }
        let (x, y) = self.translate(x, y);
        self.node
            .clone()
            .node_on_touch_motion(seat, time_usec, id, x, y);
    }

    fn frame(&self, seat: &Rc<WlSeatGlobal>) {
        self.node.node_on_touch_frame(seat);
        if self.down_ids.is_empty() {
            self.revert_to_default(seat);
        }
    }

    fn cancel(&self, seat: &Rc<WlSeatGlobal>) {
        self.node.node_on_touch_cancel(seat);
        self.revert_to_default(seat);
    }
}
