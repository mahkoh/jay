use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_surface::xdg_surface::xdg_popup::jay_popup_ext_v1::{
            JayPopupExtV1, JayPopupExtV1Error,
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{JayPopupExtManagerV1Id, jay_popup_ext_manager_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct JayPopupExtManagerV1Global {
    pub name: GlobalName,
}

pub struct JayPopupExtManagerV1 {
    pub id: JayPopupExtManagerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
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
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

global_base!(
    JayPopupExtManagerV1Global,
    JayPopupExtManagerV1,
    JayPopupExtManagerV1Error
);

impl Global for JayPopupExtManagerV1Global {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(JayPopupExtManagerV1Global);

impl JayPopupExtManagerV1RequestHandler for JayPopupExtManagerV1 {
    type Error = JayPopupExtManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
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
        self.client.add_client_obj(&obj)?;
        obj.install()?;
        Ok(())
    }
}

object_base! {
    self = JayPopupExtManagerV1;
    version = self.version;
}

impl Object for JayPopupExtManagerV1 {}

simple_add_obj!(JayPopupExtManagerV1);

#[derive(Debug, Error)]
pub enum JayPopupExtManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    JayPopupExtV1Error(#[from] JayPopupExtV1Error),
}
efrom!(JayPopupExtManagerV1Error, ClientError);
