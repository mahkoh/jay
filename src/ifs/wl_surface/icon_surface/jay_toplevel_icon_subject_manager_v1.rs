use crate::client::Client;
use crate::client::LookupError;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::wl_surface::icon_surface::jay_icon_surface_factory_v1::JayIconSurfaceFactoryV1;
use crate::ifs::wl_surface::icon_surface::jay_icon_surface_subject_v1::JayIconSurfaceSubject;
use crate::ifs::wl_surface::icon_surface::jay_icon_surface_subject_v1::JayIconSurfaceSubjectV1;
use crate::ifs::wl_surface::xdg_surface::xdg_toplevel::XdgToplevel;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::JayToplevelIconSubjectManagerV1Id;
use crate::wire::jay_toplevel_icon_subject_manager_v1::*;
use jay_proc::Global;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Global)]
pub struct JayToplevelIconSubjectManagerV1Global {
    pub name: GlobalName,
}

impl JayToplevelIconSubjectManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: JayToplevelIconSubjectManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), Infallible> {
        let obj = Rc::new(JayToplevelIconSubjectManagerV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, obj);
        client.add_client_obj(&obj);
        Ok(())
    }
}

impl Global for JayToplevelIconSubjectManagerV1Global {
    fn version(&self) -> u32 {
        1
    }
}

#[derive(Object)]
pub struct JayToplevelIconSubjectManagerV1 {
    id: JayToplevelIconSubjectManagerV1Id,
    client: Rc<Client>,
    tracker: Tracker<Self>,
    version: Version,
}

impl JayIconSurfaceSubject for XdgToplevel {
    fn has_factory(&self) -> bool {
        self.has_icon_factory()
    }

    fn set_factory(&self, factory: Option<&Rc<JayIconSurfaceFactoryV1>>) {
        self.set_icon_factory(factory);
    }
}

impl JayToplevelIconSubjectManagerV1RequestHandler for JayToplevelIconSubjectManagerV1 {
    type Error = LookupError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }

    fn create_subject(&self, req: CreateSubject, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let subject = self.client.lookup(req.toplevel)?;
        let obj = Rc::new(JayIconSurfaceSubjectV1 {
            id: req.id,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            subject,
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj);
        Ok(())
    }
}
