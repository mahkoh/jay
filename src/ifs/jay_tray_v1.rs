use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName, RemovableWaylandGlobal},
        ifs::{
            wl_output::OutputGlobalOpt,
            wl_surface::tray::jay_tray_item_v1::{JayTrayItemV1, JayTrayItemV1Error},
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{JayTrayV1Id, jay_tray_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct JayTrayV1Global {
    pub name: GlobalName,
    pub output: Rc<OutputGlobalOpt>,
}

pub struct JayTrayV1 {
    pub id: JayTrayV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub output: Rc<OutputGlobalOpt>,
}

impl JayTrayV1Global {
    fn bind_(
        self: Rc<Self>,
        id: JayTrayV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), JayTrayManagerV1Error> {
        let obj = Rc::new(JayTrayV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
            output: self.output.clone(),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        Ok(())
    }
}

global_base!(JayTrayV1Global, JayTrayV1, JayTrayManagerV1Error);

impl Global for JayTrayV1Global {
    fn singleton(&self) -> bool {
        false
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(JayTrayV1Global);

impl RemovableWaylandGlobal for JayTrayV1Global {
    fn create_replacement(self: Rc<Self>) -> Rc<dyn Global> {
        self
    }
}

impl JayTrayV1RequestHandler for JayTrayV1 {
    type Error = JayTrayManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_tray_item(&self, req: GetTrayItem, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let surface = self.client.lookup(req.surface)?;
        let fs = Rc::new(JayTrayItemV1::new(
            req.id,
            self.version,
            &surface,
            &self.output,
        ));
        track!(self.client, fs);
        fs.install()?;
        self.client.add_client_obj(&fs)?;
        Ok(())
    }
}

object_base! {
    self = JayTrayV1;
    version = self.version;
}

impl Object for JayTrayV1 {}

simple_add_obj!(JayTrayV1);

#[derive(Debug, Error)]
pub enum JayTrayManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    ExtTrayItemV1Error(#[from] JayTrayItemV1Error),
}
efrom!(JayTrayManagerV1Error, ClientError);
