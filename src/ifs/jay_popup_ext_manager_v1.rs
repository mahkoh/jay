use crate::client::Client;
use crate::client::LookupError;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::ifs::wl_surface::xdg_surface::xdg_popup::jay_popup_ext_v1::JayPopupExtV1;
use crate::ifs::wl_surface::xdg_surface::xdg_popup::jay_popup_ext_v1::JayPopupExtV1Error;
use crate::leaks::Tracker;
use crate::object::Version;
use crate::wire::JayPopupExtManagerV1Id;
use crate::wire::jay_popup_ext_manager_v1::*;
use jay_proc::Global;
use jay_proc::Object;
use std::rc::Rc;
use thiserror::Error;

#[derive(Global)]
pub struct JayPopupExtManagerV1Global {
    name: GlobalName,
}

#[derive(Object)]
pub struct JayPopupExtManagerV1 {
    id: JayPopupExtManagerV1Id,
    client: Rc<Client>,
    tracker: Tracker<Self>,
    version: Version,
}

impl JayPopupExtManagerV1Global {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: JayPopupExtManagerV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), JayPopupExtManagerV1Error> {
        let obj = Rc::new(JayPopupExtManagerV1 {
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

impl Global for JayPopupExtManagerV1Global {
    fn version(&self) -> u32 {
        1
    }
}

impl JayPopupExtManagerV1RequestHandler for JayPopupExtManagerV1 {
    type Error = JayPopupExtManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self);
        Ok(())
    }

    fn get_ext(&self, req: GetExt, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let popup = self.client.lookup(req.popup)?;
        let obj = Rc::new(JayPopupExtV1::new(
            req.id,
            &self.client,
            self.version,
            &popup,
        ));
        track!(self.client, obj);
        self.client.add_client_obj(&obj);
        obj.install()?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum JayPopupExtManagerV1Error {
    #[error(transparent)]
    Lookup(#[from] LookupError),
    #[error(transparent)]
    JayPopupExtV1Error(#[from] JayPopupExtV1Error),
}
