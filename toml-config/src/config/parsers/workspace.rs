use {
    crate::config::context::Context,
    jay_config::Workspace,
    std::{cell::Cell, fmt::Debug, rc::Rc},
};

#[derive(Debug)]
pub struct WorkspaceSlot {
    pub ws: Cell<Workspace>,
    pub ty: Cell<WorkspaceType>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum WorkspaceType {
    Normal,
}

impl Context<'_> {
    pub fn get_workspace_slot(&self, name: &str) -> Rc<WorkspaceSlot> {
        let map = &mut *self.workspaces.borrow_mut();
        if let Some(ws) = map.get(name) {
            return ws.clone();
        }
        let ws = Rc::new(WorkspaceSlot {
            ws: Cell::new(Workspace(0)),
            ty: Cell::new(WorkspaceType::Normal),
        });
        map.insert(name.to_string(), ws.clone());
        ws
    }
}
