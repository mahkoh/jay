use {
    crate::{
        fixed::Fixed,
        ifs::wl_seat::WlSeatGlobal,
        rect::Rect,
        tree::{FindTreeUsecase, FoundNode, Node},
        utils::clonecell::CloneCell,
    },
    ahash::AHashSet,
    std::{cell::RefCell, rc::Rc},
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
    pub fn down(
        &self,
        seat: &Rc<WlSeatGlobal>,
        mapped_node: Rc<dyn Node>,
        time_usec: u64,
        id: i32,
        x: Fixed,
        y: Fixed,
    ) {
        self.owner
            .get()
            .down(seat, mapped_node, time_usec, id, x, y)
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
        self.owner.set(self.default.clone());
    }
}

fn transform_abs(x: Fixed, y: Fixed, pos: Rect) -> (Fixed, Fixed) {
    let x = Fixed::from_f64(x.to_f64() * f64::from(pos.width()));
    let y = Fixed::from_f64(y.to_f64() * f64::from(pos.height()));
    (x, y)
}

fn transform_rel(x: Fixed, y: Fixed, pos: Rect) -> (Fixed, Fixed) {
    (x - pos.x1(), y - pos.y1())
}

struct DefaultTouchOwner;

struct GrabTouchOwner {
    pos: Rect,
    node: Rc<dyn Node>,
    down_ids: RefCell<AHashSet<i32>>,
}

trait TouchOwner {
    fn down(
        &self,
        seat: &Rc<WlSeatGlobal>,
        mapped_node: Rc<dyn Node>,
        time_usec: u64,
        id: i32,
        x: Fixed,
        y: Fixed,
    );
    fn up(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, id: i32);
    fn motion(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, id: i32, x: Fixed, y: Fixed);
    fn frame(&self, seat: &Rc<WlSeatGlobal>);
    fn cancel(&self, seat: &Rc<WlSeatGlobal>);
}

impl TouchOwner for DefaultTouchOwner {
    fn down(
        &self,
        seat: &Rc<WlSeatGlobal>,
        mapped_node: Rc<dyn Node>,
        time_usec: u64,
        id: i32,
        x: Fixed,
        y: Fixed,
    ) {
        seat.cursor_group().deactivate();
        let pos = mapped_node.node_absolute_position();
        let (x, y) = transform_abs(x, y, pos);
        let mut found_tree = seat.touch_found_tree.borrow_mut();
        let x_int = x.round_down();
        let y_int = y.round_down();
        found_tree.push(FoundNode {
            node: mapped_node.clone(),
            x: x_int,
            y: y_int,
        });
        mapped_node.node_find_tree_at(x_int, y_int, &mut found_tree, FindTreeUsecase::None);
        if let Some(node) = found_tree.last() {
            let node = node.node.clone();
            node.node_seat_state().touch_begin(seat);
            let down_ids = RefCell::new(AHashSet::new());
            down_ids.borrow_mut().insert(id);
            let (x_rel, y_rel) = transform_rel(x, y, node.node_absolute_position());
            seat.touch_owner.owner.set(Rc::new(GrabTouchOwner {
                pos,
                node: node.clone(),
                down_ids,
            }));
            node.node_on_touch_down(seat, time_usec, id, x_rel, y_rel);
        }
        found_tree.clear();
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

impl TouchOwner for GrabTouchOwner {
    fn down(
        &self,
        seat: &Rc<WlSeatGlobal>,
        _mapped_node: Rc<dyn Node>,
        time_usec: u64,
        id: i32,
        x: Fixed,
        y: Fixed,
    ) {
        self.down_ids.borrow_mut().insert(id);
        let (x, y) = transform_abs(x, y, self.pos);
        let (x_rel, y_rel) = transform_rel(x, y, self.node.node_absolute_position());
        self.node
            .clone()
            .node_on_touch_down(seat, time_usec, id, x_rel, y_rel);
    }

    fn up(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, id: i32) {
        self.down_ids.borrow_mut().remove(&id);
        self.node.clone().node_on_touch_up(seat, time_usec, id);
        if self.down_ids.borrow().is_empty() {
            self.node.node_seat_state().touch_end(seat);
            seat.touch_owner.clear();
        }
    }

    fn motion(&self, seat: &Rc<WlSeatGlobal>, time_usec: u64, id: i32, x: Fixed, y: Fixed) {
        self.down_ids.borrow_mut().insert(id);
        let (x, y) = transform_abs(x, y, self.pos);
        let (x_rel, y_rel) = transform_rel(x, y, self.node.node_absolute_position());
        self.node
            .clone()
            .node_on_touch_motion(seat, time_usec, id, x_rel, y_rel);
    }

    fn frame(&self, seat: &Rc<WlSeatGlobal>) {
        self.node.node_on_touch_frame(seat);
    }

    fn cancel(&self, seat: &Rc<WlSeatGlobal>) {
        self.node.node_on_touch_cancel(seat);
        self.node.node_seat_state().touch_end(seat);
        seat.touch_owner.clear();
    }
}
