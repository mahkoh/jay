use {
    crate::{
        fixed::Fixed,
        ifs::wl_seat::tablet::{TabletTool, TabletToolChanges, ToolButtonState},
        tree::{FindTreeUsecase, FoundNode, Node},
        utils::{clonecell::CloneCell, smallmap::SmallMap},
    },
    std::rc::Rc,
};

pub struct ToolOwnerHolder {
    default: Rc<DefaultToolOwner>,
    owner: CloneCell<Rc<dyn ToolOwner>>,
}

struct DefaultToolOwner;

struct GrabToolOwner {
    buttons: SmallMap<u32, (), 4>,
    node: Rc<dyn Node>,
}

impl Default for ToolOwnerHolder {
    fn default() -> Self {
        let default = Rc::new(DefaultToolOwner);
        Self {
            owner: CloneCell::new(default.clone()),
            default,
        }
    }
}

impl ToolOwnerHolder {
    pub fn destroy(&self, tool: &Rc<TabletTool>) {
        let root = tool.tablet.seat.state.root.clone();
        let prev = tool.node.set(root);
        prev.node_on_tablet_tool_leave(tool, tool.tablet.seat.state.now_usec());
        prev.node_seat_state().remove_tablet_tool_focus(tool);
    }

    pub fn focus_root(&self, tool: &Rc<TabletTool>) {
        self.owner.set(self.default.clone());
        let state = &tool.tablet.seat.state;
        let root = state.root.clone();
        tool.set_node(root, state.now_usec());
    }

    pub fn button(
        &self,
        tool: &Rc<TabletTool>,
        time_usec: u64,
        button: u32,
        state: ToolButtonState,
    ) {
        self.owner.get().button(tool, time_usec, button, state);
    }

    pub fn apply_changes(
        &self,
        tool: &Rc<TabletTool>,
        time_usec: u64,
        changes: Option<&TabletToolChanges>,
    ) {
        self.owner.get().apply_changes(tool, time_usec, changes);
    }
}

trait ToolOwner {
    fn button(&self, tool: &Rc<TabletTool>, time_usec: u64, button: u32, state: ToolButtonState);
    fn apply_changes(
        &self,
        tool: &Rc<TabletTool>,
        time_usec: u64,
        changes: Option<&TabletToolChanges>,
    );
}

impl TabletTool {
    fn set_node(self: &Rc<Self>, node: Rc<dyn Node>, time_usec: u64) {
        let prev = self.node.set(node.clone());
        if prev.node_id() != node.node_id() {
            prev.node_on_tablet_tool_leave(self, time_usec);
            prev.node_seat_state().remove_tablet_tool_focus(self);
            let (tool_x, tool_y) = self.cursor.position();
            let (node_x, node_y) = node.node_absolute_position().position();
            node.node_seat_state().add_tablet_tool_focus(self);
            node.node_on_tablet_tool_enter(self, time_usec, tool_x - node_x, tool_y - node_y);
        }
    }
}

impl ToolOwner for DefaultToolOwner {
    fn button(&self, tool: &Rc<TabletTool>, time_usec: u64, button: u32, state: ToolButtonState) {
        if state == ToolButtonState::Released {
            return;
        }
        let node = tool.node.get();
        node.node_restack();
        let owner = Rc::new(GrabToolOwner {
            buttons: Default::default(),
            node,
        });
        tool.tool_owner.owner.set(owner.clone());
        owner.button(tool, time_usec, button, state);
    }

    fn apply_changes(
        &self,
        tool: &Rc<TabletTool>,
        time_usec: u64,
        changes: Option<&TabletToolChanges>,
    ) {
        let change = handle_position_change(tool);
        let node = change.node;
        if change.changed {
            tool.set_node(node.clone(), time_usec);
        } else {
            node.clone()
                .node_on_tablet_tool_apply_changes(tool, time_usec, changes, change.x, change.y);
        }
        if tool.down.get() {
            tool.tool_owner.owner.set(Rc::new(GrabToolOwner {
                buttons: Default::default(),
                node,
            }));
        }
    }
}

impl GrabToolOwner {
    fn maybe_revert(&self, tool: &Rc<TabletTool>) {
        if !tool.down.get() && self.buttons.is_empty() {
            tool.tool_owner.owner.set(tool.tool_owner.default.clone());
            tool.tablet.seat.tree_changed.trigger();
        }
    }
}

impl ToolOwner for GrabToolOwner {
    fn button(&self, tool: &Rc<TabletTool>, time_usec: u64, button: u32, state: ToolButtonState) {
        match state {
            ToolButtonState::Released => {
                if self.buttons.remove(&button).is_none() {
                    return;
                }
            }
            ToolButtonState::Pressed => {
                if self.buttons.insert(button, ()).is_some() {
                    return;
                }
            }
        }
        self.node
            .node_on_tablet_tool_button(tool, time_usec, button, state);
        self.maybe_revert(tool);
    }

    fn apply_changes(
        &self,
        tool: &Rc<TabletTool>,
        time_usec: u64,
        changes: Option<&TabletToolChanges>,
    ) {
        let (x, y) = tool.cursor.position();
        let node_pos = self.node.node_absolute_position();
        self.node.clone().node_on_tablet_tool_apply_changes(
            tool,
            time_usec,
            changes,
            x - node_pos.x1(),
            y - node_pos.y1(),
        );
        self.maybe_revert(tool);
    }
}

fn handle_position_change(tool: &Rc<TabletTool>) -> UpdatedNode {
    let (x, y) = tool.cursor.position();
    let x_int = x.round_down();
    let y_int = y.round_down();
    let tree = &mut *tool.tablet.tree.borrow_mut();
    tree.push(FoundNode {
        node: tool.tablet.seat.state.root.clone(),
        x: x_int,
        y: y_int,
    });
    tool.tablet
        .seat
        .state
        .root
        .node_find_tree_at(x_int, y_int, tree, FindTreeUsecase::None);
    let mut update = UpdatedNode {
        node: tool.node.get(),
        x,
        y,
        changed: false,
    };
    if let Some(last) = tree.last() {
        if last.node.node_id() != update.node.node_id() {
            update.changed = true;
            update.node = last.node.clone();
        }
        update.x = x.apply_fract(last.x);
        update.y = y.apply_fract(last.y);
    }
    tree.clear();
    update
}

struct UpdatedNode {
    node: Rc<dyn Node>,
    changed: bool,
    x: Fixed,
    y: Fixed,
}
