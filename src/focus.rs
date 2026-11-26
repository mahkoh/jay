use crate::ifs::wl_seat::WlSeatGlobal;
use crate::tree::Direction;
use crate::tree::Node;
use crate::tree::TreeLink;
use crate::tree::TreeTimeline::LiveTL;
use crate::tree::toplevel_parent_container;
use crate::utils::linkedlist::LinkedList;
use crate::utils::linkedlist::NodeRef;
pub use jay_config::focus::EdgeMode;
pub use jay_config::focus::Kind;
pub use jay_config::focus::Target;
use std::iter::successors;
use std::rc::Rc;

// TODO: nicer way of doing this? Or are we special enough in needing this for non-toplevels?
fn parent(node: &Rc<dyn Node>) -> Option<Rc<dyn Node>> {
    if let Some(tl) = node.clone().node_into_toplevel() {
        if let Some(pn) = toplevel_parent_container(&*tl) {
            Some(pn)
        } else if let Some(wn) = tl.tl_data().workspace[LiveTL].get() {
            Some(wn.clone())
        } else {
            // TODO: figure out if this can actually happen
            log::info!("toplevel in focus chain without workspace?");
            None
        }
    } else if let Some(wn) = node.clone().node_into_workspace() {
        Some(wn.node_state[LiveTL].output.get())
    } else if node.node_is_output() {
        Some(node.node_output()?.state.root.clone())
    } else {
        if !node.node_is_display() {
            log::info!("focus: no parent for {}", node.node_id());
        }
        None
    }
}

/// Change focus from `tl` based on `targets`. Returns true if focus actually changed.
pub fn focus_targets(seat: &Rc<WlSeatGlobal>, node: Rc<dyn Node>, targets: &[Target]) -> bool {
    log::debug!("focus: {targets:?}");
    let parents: Vec<_> = successors(Some(node), parent).collect();
    for [child, parent] in parents.array_windows() {
        log::debug!("Parent {}...", parent.node_id());
        if let Some(target) = match_targets(&parent, targets) {
            log::debug!("...matched {target:?}");
            if let Some(neighbor) = neighbor_local(&parent, &child, target) {
                log::debug!("Found neighbor {}", neighbor.node_id());
                select_leaf(seat, neighbor, targets);
                return true;
            } else {
                if target.edge_mode == EdgeMode::Stop {
                    log::debug!("No neighbor, edge_mode stop: no focus change");
                    break;
                }
            }
        }
    }

    false
}

fn match_targets(parent: &Rc<dyn Node>, targets: &[Target]) -> Option<Target> {
    // TODO: float_focused stuff.
    //
    // TODO: rework commentary here
    //
    // In i3 / Sway, workspaces themselves act as containers (they have a layout and multiple
    // children).
    //
    // In Jay, workspaces contain a single top-level container and/or fullspace client. However, an
    // empty workspace has no container.
    //
    // We match against parents (if we match a target against a node, that means we should (try to)
    // select a new child of that node, not a new sibling). For focus_targets we could go either
    // way, but for select_leaf this way is much nicer.
    //
    // We could _almost_ get away with matching against parents for containers, nodes themselves for
    // outputs and workspaces (which I think select_leaf could handle)... if it wasn't for the
    // "empty workspace" edge case. An empty workspace has no container, so focus_targets (starting
    // at the first parent) would skip that level.
    let container = parent.clone().node_into_container();
    targets.iter().copied().find(|target| match target.kind {
        Kind::Output(_) => parent.node_is_display(),
        Kind::Workspace => parent.node_is_output(),
        Kind::Split(axis) => match &container {
            None => false,
            Some(node) => {
                let ns = &node.node_state[LiveTL];
                ns.mono_child.get().is_none() && ns.split.get() == axis.into()
            }
        },
        Kind::Mono => match &container {
            None => false,
            Some(cn) => cn.node_state[LiveTL].mono_child.get().is_some(),
        },
        Kind::Float => false, // TODO
    })
}

fn select_child<T>(
    list: &LinkedList<TreeLink<T>>,
    target: Target,
    pred: impl Fn(&NodeRef<TreeLink<T>>) -> bool,
) -> Option<NodeRef<TreeLink<T>>> {
    let Some(current_link) = list.iter_valid(LiveTL).find(pred) else {
        log::warn!("select_child did not find current child");
        return None;
    };
    let maybe_next_link = if target.backward {
        current_link.prev_valid(LiveTL)
    } else {
        current_link.next_valid(LiveTL)
    };
    if maybe_next_link.is_none() && target.edge_mode == EdgeMode::Wrap {
        if target.backward {
            list.rev_iter_valid(LiveTL).next()
        } else {
            list.iter_valid(LiveTL).next()
        }
    } else {
        maybe_next_link
    }
}

fn neighbor_local(
    parent: &Rc<dyn Node>,
    child: &Rc<dyn Node>,
    target: Target,
) -> Option<Rc<dyn Node>> {
    match target.kind {
        Kind::Float => None, // TODO
        Kind::Output(direction) => {
            let on = child.node_output()?;
            // TODO: wrapping
            // TODO: "include focused container as a last resort", if relevant for Output
            on.state
                .find_output_in_direction(&on, direction.into())
                .map(|n| n as Rc<dyn Node>)
        }
        Kind::Workspace => {
            let workspaces = &child.node_output()?.workspaces;
            let current = child.node_workspace()?;
            let node = select_child(workspaces, target, |n| n.id == current.id)?;
            Some((**node).clone())
        }
        Kind::Split(_) | Kind::Mono => {
            let container = parent.clone().node_into_container()?;
            // TODO (attempt to) verify last_active is actually correct...
            // (also used again below in select_leaf)
            let current = container.last_active()?;
            let node = select_child(&container.children, target, |n| n.node.node_id() == current)?;
            // Needed for mono containers, noop otherwise.
            container.activate_child(&node);
            Some(node.node.clone())
        }
    }
}

/// Find a leaf in a (presumed) neighboring container, respecting target edge-modes
fn select_leaf(seat: &Rc<WlSeatGlobal>, node: Rc<dyn Node>, targets: &[Target]) {
    successors(Some(node), |node| {
        let target = match_targets(node, targets);
        if node.node_is_output() {
            let on = node.node_output()?;
            if let Some(target) = target
                && target.edge_mode == EdgeMode::Traverse
            {
                let link = if target.backward {
                    on.workspaces.rev_iter_valid(LiveTL).next()?
                } else {
                    on.workspaces.iter_valid(LiveTL).next()?
                };
                return Some((**link).clone());
            }
            return Some(on.workspace()? as Rc<dyn Node>);
        }

        let cn = if node.node_is_workspace() {
            node.clone().node_into_workspace()?.node_state[LiveTL]
                .container
                .get()?
        } else if node.node_is_container() {
            node.clone().node_into_container()?
        } else {
            // TODO: floats, somewhere...

            return None;
        };

        let current = cn.last_active()?;

        // TODO: refactor out common code, if it stays the same
        let child = if let Some(target) = target
            && target.edge_mode == EdgeMode::Traverse
        {
            if target.backward {
                cn.children.rev_iter_valid(LiveTL).next()?
            } else {
                cn.children.iter_valid(LiveTL).next()?
            }
        } else {
            cn.children
                .iter_valid(LiveTL)
                .find(|n| n.node.node_id() == current)?
        };
        log::debug!("activating child: {}", child.node.node_id());
        // Necessary for mono containers if this child is not current. Noop otherwise.
        cn.activate_child(&child);
        return Some(child.node.clone());
    })
    .inspect(|node| log::debug!("descended to {}", node.node_id()))
    .last()
    // Safe because we call successors with a non-None starting value.
    .unwrap()
    .node_do_focus_dyn(seat, Direction::Unspecified);
}
