use crate::client::Client;
use crate::client::LookupError;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::jay_wl_surface_factory_v1::JayWlSurfaceFactoryV1Error;
use crate::ifs::wl_surface::icon_surface::jay_icon_surface_factory_v1::JayIconSurfaceFactoryV1;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::utils::clonecell::CloneCell;
use crate::wire::JayIconSurfaceManagerV1Id;
use crate::wire::jay_icon_surface_manager_v1::*;
use jay_proc::Global;
use jay_proc::Object;
use std::rc::Rc;
use thiserror::Error;

#[derive(Global)]
pub struct JayIconSurfaceManagerV1Global {
    pub name: GlobalName,
}

impl JayIconSurfaceManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: JayIconSurfaceManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), JayIconSurfaceManagerV1Error> {
        let obj = Rc::new(JayIconSurfaceManagerV1 {
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

impl Global for JayIconSurfaceManagerV1Global {
    fn version(&self) -> u32 {
        1
    }
}

#[derive(Object)]
pub struct JayIconSurfaceManagerV1 {
    id: JayIconSurfaceManagerV1Id,
    client: Rc<Client>,
    tracker: Tracker<Self>,
    version: Version,
}

impl JayIconSurfaceManagerV1RequestHandler for JayIconSurfaceManagerV1 {
    type Error = JayIconSurfaceManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }

    fn create_factory(&self, req: CreateFactory, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let surface_factory = self.client.lookup(req.surface_factory)?;
        surface_factory.assign()?;
        let subject = &self.client.lookup(req.subject)?.subject;
        if subject.has_factory() {
            return Err(JayIconSurfaceManagerV1Error::Exists);
        }
        let obj = Rc::new(JayIconSurfaceFactoryV1 {
            id: req.id,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            subject: CloneCell::new(Some(subject.clone())),
            surface_factory,
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj);
        subject.set_factory(Some(&obj));
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum JayIconSurfaceManagerV1Error {
    #[error(transparent)]
    Lookup(#[from] LookupError),
    #[error("The subject already has a factory")]
    Exists,
    #[error(transparent)]
    WlSurfaceFactory(#[from] JayWlSurfaceFactoryV1Error),
}
