use crate::backend::Output;
use crate::ifs::wl_output::WlOutputGlobal;
use crate::tree::{Node, OutputNode, WorkspaceNode};
use crate::utils::asyncevent::AsyncEvent;
use crate::utils::clonecell::CloneCell;
use crate::State;
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
        let on = Rc::new(OutputNode {
            display: self.state.root.clone(),
            id: self.state.node_ids.next(),
            backend: self.output.clone(),
            workspaces: RefCell::new(vec![]),
            position: Cell::new(Default::default()),
            workspace: CloneCell::new(None),
        });
        let workspace = Rc::new(WorkspaceNode {
            id: self.state.node_ids.next(),
            output: CloneCell::new(on.clone()),
            container: Default::default(),
            floaters: Default::default(),
        });
        on.workspace.set(Some(workspace));
        self.state.root.outputs.set(self.output.id(), on.clone());
        let name = self.state.globals.name();
        let global = Rc::new(WlOutputGlobal::new(name, &self.output));
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
                on.clone().change_size(width, height);
            }
            global.update_properties();
            ae.triggered().await;
        }
        self.state.outputs.remove(&self.output.id());
        let _ = self.state.globals.remove(&self.state, name);
        self.state
            .output_handlers
            .borrow_mut()
            .remove(&self.output.id());
        self.state.root.outputs.remove(&self.output.id());
    }
}
