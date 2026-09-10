use crate::client::Client;
use crate::ifs::wl_surface::icon_surface::jay_icon_surface_factory_v1::JayIconSurfaceFactoryV1;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::JayIconSurfaceSubjectV1Id;
use crate::wire::jay_icon_surface_subject_v1::*;
use jay_proc::Object;
use std::convert::Infallible;
use std::rc::Rc;

#[derive(Object)]
pub struct JayIconSurfaceSubjectV1 {
    pub id: JayIconSurfaceSubjectV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub subject: Rc<dyn JayIconSurfaceSubject>,
}

pub trait JayIconSurfaceSubject {
    fn has_factory(&self) -> bool;
    fn set_factory(&self, factory: Option<&Rc<JayIconSurfaceFactoryV1>>);
}

impl JayIconSurfaceSubjectV1RequestHandler for JayIconSurfaceSubjectV1 {
    type Error = Infallible;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }
}
