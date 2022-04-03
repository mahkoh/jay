use crate::backend::{Connector, ConnectorEvent};
use crate::ifs::wl_output::WlOutputGlobal;
use crate::rect::Rect;
use crate::state::State;
use crate::tree::{OutputNode, OutputRenderData, WorkspaceNode};
use crate::utils::asyncevent::AsyncEvent;
use crate::utils::clonecell::CloneCell;
use std::cell::{RefCell};
use std::rc::Rc;

pub struct OutputHandler {
    pub state: Rc<State>,
    pub output: Rc<dyn Connector>,
}

impl OutputHandler {
    pub async fn handle(self) {
        let ae = Rc::new(AsyncEvent::default());
        {
            let ae = ae.clone();
            self.output.on_change(Rc::new(move || ae.trigger()));
        }
        let name = self.state.globals.name();
        let x1 = self.state.root.outputs.lock().values().map(|o| o.global.pos.get().x2()).max().unwrap_or(0);
        let global = Rc::new(WlOutputGlobal::new(name, self.output.clone(), x1));
        let on = Rc::new(OutputNode {
            id: self.state.node_ids.next(),
            workspaces: Default::default(),
            workspace: CloneCell::new(None),
            seat_state: Default::default(),
            global: global.clone(),
            layers: Default::default(),
            render_data: RefCell::new(OutputRenderData {
                active_workspace: Rect::new_empty(0, 0),
                inactive_workspaces: Default::default(),
                titles: Default::default(),
            }),
            state: self.state.clone(),
            is_dummy: false,
        });
        global.node.set(Some(on.clone()));
        let name = 'name: {
            for i in 1.. {
                let name = i.to_string();
                if !self.state.workspaces.contains(&name) {
                    break 'name name;
                }
            }
            unreachable!();
        };
        let workspace = Rc::new(WorkspaceNode {
            id: self.state.node_ids.next(),
            output: CloneCell::new(on.clone()),
            position: Default::default(),
            container: Default::default(),
            stacked: Default::default(),
            seat_state: Default::default(),
            name: name.clone(),
            output_link: Default::default(),
        });
        self.state.workspaces.set(name, workspace.clone());
        workspace
            .output_link
            .set(Some(on.workspaces.add_last(workspace.clone())));
        on.show_workspace(&workspace);
        on.update_render_data();
        self.state.root.outputs.set(self.output.id(), on.clone());
        self.state.add_global(&global);
        self.state.outputs.set(self.output.id(), global.clone());
        'outer: loop {
            while let Some(event) = self.output.event() {
                match event {
                    ConnectorEvent::Removed => break 'outer,
                    ConnectorEvent::ModeChanged(mode) => {
                        on.update_mode(mode);
                    }
                }
            }
            ae.triggered().await;
        }
        global.node.set(None);
        self.state.outputs.remove(&self.output.id());
        let _ = self.state.remove_global(&*global);
        self.state
            .output_handlers
            .borrow_mut()
            .remove(&self.output.id());
        self.state.root.outputs.remove(&self.output.id());
    }
}
