use crate::tree::NodeId;
use std::rc::Rc;

pub mod jay_icon_surface_factory_v1;
pub mod jay_icon_surface_manager_v1;
pub mod jay_icon_surface_subject_v1;
pub mod jay_icon_surface_v1;

pub trait IconSurfaceOwner {
    fn destroyed(self: Rc<Self>, node_id: NodeId);
}
