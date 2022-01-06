use crate::backend::{Output, OutputId};
use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::XdgToplevel;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::linkedlist::{LinkedList, Node as LinkedNode};
use ahash::AHashMap;
use std::cell::{Cell, RefCell};
use std::mem;
use std::rc::Rc;

linear_ids!(NodeIds, NodeId);

pub trait NodeBase {
    fn id(&self) -> NodeId;
    fn parent(&self) -> Option<Rc<dyn Node>>;
    fn extents(&self) -> NodeExtents;
}

macro_rules! base {
    ($name:ident) => {
        impl NodeBase for $name {
            fn id(&self) -> NodeId {
                self.common.id
            }

            fn parent(&self) -> Option<Rc<dyn Node>> {
                self.common.parent.clone()
            }

            fn extents(&self) -> NodeExtents {
                self.common.extents.get()
            }
        }
    };
}

pub trait Node: NodeBase {
    fn into_kind(self: Rc<Self>) -> NodeKind;
    fn clear(&self);
}

pub enum NodeKind {
    Display(Rc<DisplayNode>),
    Output(Rc<OutputNode>),
    Toplevel(Rc<ToplevelNode>),
    Container(Rc<ContainerNode>),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub struct NodeExtents {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

pub struct NodeCommon {
    pub extents: Cell<NodeExtents>,
    pub id: NodeId,
    pub parent: Option<Rc<dyn Node>>,
    pub floating_outputs: RefCell<AHashMap<NodeId, LinkedNode<Rc<dyn Node>>>>,
}

impl NodeCommon {
    fn clear(&self) {
        mem::take(&mut *self.floating_outputs.borrow_mut());
    }
}

pub struct DisplayNode {
    pub common: NodeCommon,
    pub outputs: CopyHashMap<OutputId, Rc<OutputNode>>,
}

impl DisplayNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            common: NodeCommon {
                extents: Default::default(),
                id,
                parent: None,
                floating_outputs: Default::default(),
            },
            outputs: Default::default(),
        }
    }
}

base!(DisplayNode);

impl Node for DisplayNode {
    fn into_kind(self: Rc<Self>) -> NodeKind {
        NodeKind::Display(self)
    }

    fn clear(&self) {
        self.common.clear();
        let mut outputs = self.outputs.lock();
        for output in outputs.values() {
            output.clear();
        }
        outputs.clear();
    }
}

pub struct OutputNode {
    pub common: NodeCommon,
    pub backend: Rc<dyn Output>,
    pub child: RefCell<Option<Rc<dyn Node>>>,
    pub floating: LinkedList<Rc<dyn Node>>,
}

base!(OutputNode);

impl Node for OutputNode {
    fn into_kind(self: Rc<Self>) -> NodeKind {
        NodeKind::Output(self)
    }

    fn clear(&self) {
        self.common.clear();
        for floating in self.floating.iter() {
            floating.clear();
        }
        if let Some(child) = self.child.borrow_mut().take() {
            child.clear();
        }
    }
}

pub struct ToplevelNode {
    pub common: NodeCommon,
    pub surface: Rc<XdgToplevel>,
}

base!(ToplevelNode);

impl Node for ToplevelNode {
    fn into_kind(self: Rc<Self>) -> NodeKind {
        NodeKind::Toplevel(self)
    }

    fn clear(&self) {
        self.common.clear();
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ContainerSplit {
    Horizontal,
    Vertical,
}

pub struct ContainerNode {
    pub common: NodeCommon,
    pub split: Cell<ContainerSplit>,
    pub children: LinkedList<Rc<dyn Node>>,
}

base!(ContainerNode);

impl Node for ContainerNode {
    fn into_kind(self: Rc<Self>) -> NodeKind {
        NodeKind::Container(self)
    }

    fn clear(&self) {
        for child in self.children.iter() {
            child.clear();
        }
    }
}
