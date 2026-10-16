use crate::ifs::wl_seat::WlSeatGlobal;
use crate::tree::ContainerSplit;
use crate::tree::Direction;
use crate::tree::Node;
use crate::tree::ToplevelNode;
use crate::tree::TreeTimeline::LiveTL;
use crate::tree::toplevel_parent_container;
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
    } else {
        None
    }
}

pub fn neighbor(
    seat: &Rc<WlSeatGlobal>,
    tl: Rc<dyn ToplevelNode>,
    targets: &[Target],
) -> Option<Rc<dyn Node>> {
    log::debug!("focus: {targets:?}");
    for node in successors(Some(tl as Rc<dyn Node>), parent) {
        log::debug!("Child {}...", node.node_id());
        if let Some(target) = match_targets(&node, targets) {
            log::debug!("...matched {target:?}");
            if let Some(neighbor) = neighbor_local(&node, target) {
                log::debug!("Found neighbor {}", neighbor.node_id());
                return Some(select_leaf(seat, neighbor, targets));
            } else {
                if target.edge_mode == EdgeMode::Stop {
                    log::debug!("No neighbor, edge_mode stop: no focus change");
                    break;
                }
            }
        }
    }

    None
}

fn match_targets(node: &Rc<dyn Node>, targets: &[Target]) -> Option<Target> {
    // TODO: float_focused stuff.
    //
    // TODO: rework commentary here
    //
    // In i3 / Sway, workspaces themselves act as containers (they have a layout and multiple
    // children).
    //
    // In Jay, workspaces contain a single top-level container.
    //
    // In overfocus, this function selects _parents_ for the target type: containers or
    // workspaces for split etc, output for workspace targets, root for output targets.
    // neighbor_local then gets passed that parent node and selects the next focused child.
    //
    // In Jay, I think we need the focused child: unlike Sway / swayipc we can trivially get its
    // parent but we cannot trivially get the parent's focused child later.
    //
    // So this diverges from overfocus, matching if _this node_ is an output / workspace, or _its
    // container_ is of the target kind.
    let container = node
        .clone()
        .node_into_toplevel()
        .as_deref()
        .and_then(toplevel_parent_container);
    let res = *targets.iter().find(|target| match target.kind {
        Kind::Output => node.node_is_output(),
        Kind::Workspace => node.node_is_workspace(),
        Kind::Split => match &container {
            None => false,
            Some(node) => {
                let ns = &node.node_state[LiveTL];
                if ns.mono_child.get().is_some() {
                    false
                } else {
                    let split = ns.split.get();
                    if target.vertical {
                        split == ContainerSplit::Vertical
                    } else {
                        split == ContainerSplit::Horizontal
                    }
                }
            }
        },
        Kind::Mono => match &container {
            None => false,
            Some(cn) => cn.node_state[LiveTL].mono_child.get().is_some(),
        },
        Kind::Float => false, // TODO
    })?;

    Some(res)
}

fn neighbor_local(node: &Rc<dyn Node>, target: Target) -> Option<Rc<dyn Node>> {
    // TODO: remove this commentary once all working
    //
    // In overfocus, this selects a neighbor of the focused child _of_ `node`.
    // In Jay, this selects a neighbor of `node` itself.
    match target.kind {
        Kind::Float => None, // TODO
        Kind::Output => {
            let dir = match (target.vertical, target.backward) {
                (true, true) => Direction::Up,
                (true, false) => Direction::Down,
                (false, true) => Direction::Left,
                (false, false) => Direction::Right,
            };
            let on = node.node_output()?;
            // TODO: wrapping
            // TODO: "include focused container as a last resort", if relevant for Output
            on.state
                .find_output_in_direction(&on, dir)
                .map(|n| n as Rc<dyn Node>)
        }
        Kind::Workspace => {
            let workspaces = &node.node_output()?.workspaces;
            let current = node.node_workspace()?;

            let link = workspaces.iter_valid(LiveTL).find(|n| n.id == current.id)?;
            let next_link = match if target.backward {
                link.prev_valid(LiveTL)
            } else {
                link.next_valid(LiveTL)
            } {
                Some(link) => link,
                None => {
                    if target.edge_mode != EdgeMode::Wrap {
                        return None;
                    }
                    if target.backward {
                        workspaces.rev_iter_valid(LiveTL).next()?
                    } else {
                        workspaces.iter_valid(LiveTL).next()?
                    }
                }
            };
            Some((**next_link).clone())
        }
        Kind::Split | Kind::Mono => {
            // TODO: try introducing a MatchedTarget that carries the toplevel back down for us.
            let container = node
                .clone()
                .node_into_toplevel()
                .as_deref()
                .and_then(toplevel_parent_container)?;
            // TODO (attempt to) verify last_active is actually correct...
            // (also used again below in select_leaf)
            let current = container.last_active()?;
            let children = &container.children;

            // TODO: refactor out common code, if it stays the same
            let link = children
                .iter_valid(LiveTL)
                .find(|n| n.node.node_id() == current)?;
            let next_link = match if target.backward {
                link.prev_valid(LiveTL)
            } else {
                link.next_valid(LiveTL)
            } {
                Some(link) => link,
                None => {
                    if target.edge_mode != EdgeMode::Wrap {
                        return None;
                    }
                    if target.backward {
                        children.rev_iter_valid(LiveTL).next()?
                    } else {
                        children.iter_valid(LiveTL).next()?
                    }
                }
            };
            Some(next_link.node.clone())
        }
    }
}

/// Find a leaf in a (presumed) neighboring container, respecting target edge-modes
fn select_leaf(seat: &Rc<WlSeatGlobal>, node: Rc<dyn Node>, targets: &[Target]) -> Rc<dyn Node> {
    // TODO: revise commentary once confirmed working.
    //
    // This diverges from overfocus because our match_targets takes the child node.
    // Instead of calling match_targets right away, we get the selected child first.
    // Then we use that to call match_targets and change our pick if necessary.
    let leaf = successors(Some(node), |node| {
        if node.node_is_output() {
            // TODO: actually focus the output if it has no workspace.
            let on = node.node_output()?;
            let workspace = on.workspace()?;
            if let Some(target) = match_targets(&(workspace.clone() as Rc<dyn Node>), targets)
                && target.edge_mode == EdgeMode::Traverse
            {
                return Some(
                    (**(if target.backward {
                        on.workspaces.rev_iter_valid(LiveTL).next()?
                    } else {
                        on.workspaces.iter_valid(LiveTL).next()?
                    }))
                    .clone(),
                );
            }
            return Some(workspace);
        }

        let cn = if node.node_is_workspace() {
            // TODO: actually focus the workspace if it has no children.
            node.clone().node_into_workspace()?.node_state[LiveTL]
                .container
                .get()?
        } else if node.node_is_container() {
            node.clone().node_into_container()?
        } else {
            // TODO: floats, somewhere...

            // TODO: refactor actual focusing calls (duplicated below).
            if let Some(tl) = node.clone().node_into_toplevel()
                && let Some(cn) = toplevel_parent_container(&*tl)
            {
                let link = cn
                    .children
                    .iter_valid(LiveTL)
                    .find(|n| n.node.node_id() == tl.node_id())?;
                cn.activate_child(&link);
                tl.node_do_focus_dyn(seat, Direction::Unspecified);
            }
            return None;
        };

        let current = cn.last_active()?;

        // TODO: refactor out common code, if it stays the same
        let link = cn
            .children
            .iter_valid(LiveTL)
            .find(|n| n.node.node_id() == current)?;
        // TODO: after selecting a child, we need to call activate_child() on its container and may
        // need to call node_do_focus_dyn.
        let child = if let Some(target) =
            match_targets(&(link.node.clone() as Rc<dyn Node>), targets)
            && target.edge_mode == EdgeMode::Traverse
        {
            if target.backward {
                cn.children.rev_iter_valid(LiveTL).next()?
            } else {
                cn.children.iter_valid(LiveTL).next()?
            }
        } else {
            link
        };
        // TODO: this calls both activate_child and node_do_focus_dyn.
        // In move_focus_from_child (which is where move_focus normally ends up),
        // we call only the former for mono, only the latter otherwise.
        // activate_child is a noop for non-mono containers, so that unconditional call looks ok.
        // Same might not be true for the node_do_focus_dyn.
        log::debug!("activating child: {}", child.node.node_id());
        cn.activate_child(&child);
        child
            .node
            .clone()
            .node_do_focus_dyn(seat, Direction::Unspecified);
        return Some(child.node.clone());
    })
    .last()
    // unwrap is safe because we called successors with a non-None starting value.
    .unwrap();
    log::debug!("Selected leaf {}", leaf.node_id());
    leaf
}
