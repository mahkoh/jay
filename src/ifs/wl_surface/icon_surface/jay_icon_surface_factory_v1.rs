use crate::client::Client;
use crate::ifs::jay_wl_surface_factory_v1::JayWlSurfaceFactoryV1;
use crate::ifs::wl_surface::icon_surface::IconSurfaceOwner;
use crate::ifs::wl_surface::icon_surface::jay_icon_surface_subject_v1::JayIconSurfaceSubject;
use crate::ifs::wl_surface::icon_surface::jay_icon_surface_v1::IconSurface;
use crate::leaks::Tracker;
use crate::object::BreakLoops;
use crate::object::Version;
use crate::tree::NodeId;
use crate::utils::clonecell::CloneCell;
use crate::wire::JayIconSurfaceFactoryV1Id;
use crate::wire::JayIconSurfaceV1Id;
use crate::wire::jay_icon_surface_factory_v1::*;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Object)]
#[break_loops]
pub struct JayIconSurfaceFactoryV1 {
    pub id: JayIconSurfaceFactoryV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub subject: CloneCell<Option<Rc<dyn JayIconSurfaceSubject>>>,
    pub surface_factory: Rc<JayWlSurfaceFactoryV1>,
}

impl JayIconSurfaceFactoryV1 {
    #[expect(unused)]
    pub fn build_surface(
        &self,
        owner: &Rc<impl IconSurfaceOwner + 'static>,
        node_id: impl Into<NodeId>,
    ) -> IconSurface {
        self.build_surface_dyn(owner.clone(), node_id.into())
    }

    pub fn build_surface_dyn(
        &self,
        owner: Rc<dyn IconSurfaceOwner>,
        node_id: NodeId,
    ) -> IconSurface {
        let wl_surface = self.surface_factory.send_surface();
        let id = self.client.new_id(self);
        let surface = IconSurface::new(&self.client, id, self.version, &wl_surface, owner, node_id);
        self.send_surface(id);
        surface
    }

    fn detach(&self) {
        if let Some(subject) = self.subject.take() {
            subject.set_factory(None);
            self.surface_factory.abandon();
        }
    }

    fn send_surface(&self, id: JayIconSurfaceV1Id) {
        self.client.event(Surface {
            self_id: self.id,
            id,
        });
    }

    fn send_stopped(&self) {
        self.client.event(Stopped { self_id: self.id });
    }
}

impl JayIconSurfaceFactoryV1RequestHandler for JayIconSurfaceFactoryV1 {
    type Error = Infallible;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self);
        Ok(())
    }

    fn stop(&self, _req: Stop, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.send_stopped();
        Ok(())
    }
}

impl BreakLoops for JayIconSurfaceFactoryV1 {
    fn break_loops(self: Rc<Self>) {
        self.detach();
    }
}
