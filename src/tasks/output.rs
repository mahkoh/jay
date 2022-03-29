use crate::backend::Output;
use crate::ifs::wl_output::WlOutputGlobal;
use crate::rect::Rect;
use crate::state::State;
use crate::tree::{Node, OutputNode, WorkspaceNode};
use crate::utils::asyncevent::AsyncEvent;
use crate::utils::clonecell::CloneCell;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub struct OutputHandler {
    pub state: Rc<State>,
    pub output: Rc<dyn Output>,
}

impl OutputHandler {
    pub async fn handle(self) {
        let ae = Rc::new(AsyncEvent::default());
        {
            let ae = ae.clone();
            self.output.on_change(Rc::new(move || ae.trigger()));
        }
        let name = self.state.globals.name();
        let global = Rc::new(WlOutputGlobal::new(name, &self.output));
        let on = Rc::new(OutputNode {
            display: self.state.root.clone(),
            id: self.state.node_ids.next(),
            workspaces: RefCell::new(vec![]),
            position: Cell::new(Default::default()),
            workspace: CloneCell::new(None),
            seat_state: Default::default(),
            global: global.clone(),
            layers: Default::default(),
        });
        global.node.set(Some(on.clone()));
        let workspace = Rc::new(WorkspaceNode {
            id: self.state.node_ids.next(),
            output: CloneCell::new(on.clone()),
            container: Default::default(),
            stacked: Default::default(),
            seat_state: Default::default(),
        });
        on.workspaces.borrow_mut().push(workspace.clone());
        on.workspace.set(Some(workspace));
        self.state.root.outputs.set(self.output.id(), on.clone());
        self.state.add_global(&global);
        self.state.outputs.set(self.output.id(), global.clone());
        let mut width = 0;
        let mut height = 0;
        loop {
            if self.output.removed() {
                break;
            }
            let new_width = self.output.width();
            let new_height = self.output.height();
            if new_width != width || new_height != height {
                width = new_width;
                height = new_height;
                on.clone()
                    .change_extents(&Rect::new_sized(0, 0, new_width, new_height).unwrap());
            }
            global.update_properties();
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