use {
    crate::{
        ifs::wl_seat::tablet::{PadButtonState, TabletPad},
        tree::Node,
        utils::{clonecell::CloneCell, smallmap::SmallMap},
    },
    std::rc::Rc,
};

pub struct PadOwnerHolder {
    default: Rc<DefaultPadOwner>,
    owner: CloneCell<Rc<dyn PadOwner>>,
}

trait PadOwner {
    fn revert_to_default(&self, pad: &Rc<TabletPad>, time_usec: u64);
    fn update_node(&self, pad: &Rc<TabletPad>);
    fn button(&self, pad: &Rc<TabletPad>, time_usec: u64, button: u32, state: PadButtonState);
}

struct DefaultPadOwner;

struct GrabPadOwner {
    buttons: SmallMap<u32, (), 4>,
    node: Rc<dyn Node>,
}

impl Default for PadOwnerHolder {
    fn default() -> Self {
        let default = Rc::new(DefaultPadOwner);
        Self {
            owner: CloneCell::new(default.clone()),
            default,
        }
    }
}

impl PadOwnerHolder {
    pub fn update_node(&self, pad: &Rc<TabletPad>) {
        self.owner.get().update_node(pad);
    }

    pub fn destroy(&self, pad: &Rc<TabletPad>) {
        self.owner
            .get()
            .revert_to_default(pad, pad.seat.state.now_usec());
        let prev = pad.node.set(pad.seat.state.root.clone());
        prev.node_on_tablet_pad_leave(pad);
        prev.node_seat_state().remove_tablet_pad_focus(pad);
    }

    pub fn button(&self, pad: &Rc<TabletPad>, time_usec: u64, button: u32, state: PadButtonState) {
        self.owner.get().button(pad, time_usec, button, state);
    }

    pub fn focus_root(&self, pad: &Rc<TabletPad>) {
        self.owner
            .get()
            .revert_to_default(pad, pad.seat.state.now_usec());
        let node = pad.seat.state.root.clone();
        pad.focus_node(node);
    }

    fn set_default_owner(&self) {
        self.owner.set(self.default.clone());
    }
}

impl TabletPad {
    fn focus_node(self: &Rc<Self>, node: Rc<dyn Node>) {
        let prev = self.node.set(node.clone());
        if node.node_id() != prev.node_id() {
            prev.node_on_tablet_pad_leave(self);
            prev.node_seat_state().remove_tablet_pad_focus(self);
            node.node_seat_state().add_tablet_pad_focus(self);
            node.node_on_tablet_pad_enter(self);
        }
    }
}

impl PadOwner for DefaultPadOwner {
    fn revert_to_default(&self, _pad: &Rc<TabletPad>, _time_usec: u64) {
        // nothing
    }

    fn update_node(&self, pad: &Rc<TabletPad>) {
        let node = pad.seat.keyboard_node.get();
        pad.focus_node(node);
    }

    fn button(&self, pad: &Rc<TabletPad>, time_usec: u64, button: u32, state: PadButtonState) {
        if state != PadButtonState::Pressed {
            return;
        }
        let node = pad.node.get();
        let owner = Rc::new(GrabPadOwner {
            buttons: Default::default(),
            node,
        });
        pad.pad_owner.owner.set(owner.clone());
        owner.button(pad, time_usec, button, state);
    }
}

impl PadOwner for GrabPadOwner {
    fn revert_to_default(&self, pad: &Rc<TabletPad>, time_usec: u64) {
        for (button, _) in &self.buttons {
            self.node
                .node_on_tablet_pad_button(pad, time_usec, button, PadButtonState::Released);
        }
        pad.pad_owner.set_default_owner();
    }

    fn update_node(&self, _pad: &Rc<TabletPad>) {
        // nothing
    }

    fn button(&self, pad: &Rc<TabletPad>, time_usec: u64, button: u32, state: PadButtonState) {
        match state {
            PadButtonState::Released => {
                if self.buttons.remove(&button).is_none() {
                    return;
                }
            }
            PadButtonState::Pressed => {
                if self.buttons.insert(button, ()).is_some() {
                    return;
                }
            }
        }
        self.node
            .node_on_tablet_pad_button(pad, time_usec, button, state);
        if self.buttons.is_empty() {
            pad.pad_owner.set_default_owner();
            pad.pad_owner.default.update_node(pad);
        }
    }
}
