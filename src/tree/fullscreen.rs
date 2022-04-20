use std::cell::{Cell, RefCell};
use std::ops::Deref;
use std::rc::Rc;
use crate::tree::{PlaceholderNode, Node, WorkspaceNode, SizedNode};

pub trait SizedFullscreenNode: SizedNode {
    fn data(&self) -> &FullscreenData;
    fn on_set_fullscreen(&self, workspace: &Rc<WorkspaceNode>);
    fn on_unset_fullscreen(&self);
    fn title(&self) -> String;

    fn as_node(&self) -> &dyn Node {
        self
    }

    fn into_node(self: Rc<Self>) -> Rc<dyn Node> {
        self
    }
}

pub trait FullscreenNode {
    fn data(&self) -> &FullscreenData;
    fn on_set_fullscreen(&self, workspace: &Rc<WorkspaceNode>);
    fn on_unset_fullscreen(&self);
    fn as_node(&self) -> &dyn Node;
    fn into_node(self: Rc<Self>) -> Rc<dyn Node>;
    fn title(&self) -> String;
}

impl<T: SizedFullscreenNode> FullscreenNode for T {
    fn data(&self) -> &FullscreenData {
        <Self as SizedFullscreenNode>::data(self)
    }

    fn on_set_fullscreen(&self, workspace: &Rc<WorkspaceNode>) {
        <Self as SizedFullscreenNode>::on_set_fullscreen(self, workspace)
    }

    fn on_unset_fullscreen(&self) {
        <Self as SizedFullscreenNode>::on_unset_fullscreen(self)
    }

    fn as_node(&self) -> &dyn Node {
        <Self as SizedFullscreenNode>::as_node(self)
    }

    fn into_node(self: Rc<Self>) -> Rc<dyn Node> {
        <Self as SizedFullscreenNode>::into_node(self)
    }

    fn title(&self) -> String {
        <Self as SizedFullscreenNode>::title(self)
    }
}

pub struct FullscreenedData {
    pub placeholder: Rc<PlaceholderNode>,
    pub workspace: Rc<WorkspaceNode>,
}

#[derive(Default)]
pub struct FullscreenData {
    pub is_fullscreen: Cell<bool>,
    pub data: RefCell<Option<FullscreenedData>>,
}

impl FullscreenData {
    pub fn set_title(&self, title: &str) {
        let data = self.data.borrow_mut();
        if let Some(data) = data.deref() {
            data.placeholder.set_title(title);
        }
    }
}
