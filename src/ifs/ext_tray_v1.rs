use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName, RemovableWaylandGlobal},
        ifs::{
            wl_output::OutputGlobalOpt,
            wl_surface::tray::ext_tray_item_v1::{ExtTrayItemV1, ExtTrayItemV1Error},
        },
        leaks::Tracker,
        object::{Object, Version},
        wire::{ext_tray_v1::*, ExtTrayV1Id},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ExtTrayV1Global {
    pub name: GlobalName,
    pub output: Rc<OutputGlobalOpt>,
}

pub struct ExtTrayV1 {
    pub id: ExtTrayV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub output: Rc<OutputGlobalOpt>,
}

impl ExtTrayV1Global {
    fn bind_(
        self: Rc<Self>,
        id: ExtTrayV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), ExtTrayManagerV1Error> {
        let obj = Rc::new(ExtTrayV1 {
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

global_base!(ExtTrayV1Global, ExtTrayV1, ExtTrayManagerV1Error);

impl Global for ExtTrayV1Global {
    fn singleton(&self) -> bool {
        false
    }

    fn version(&self) -> u32 {
        1
    }
}

simple_add_global!(ExtTrayV1Global);

impl RemovableWaylandGlobal for ExtTrayV1Global {
    fn create_replacement(self: Rc<Self>) -> Rc<dyn Global> {
        self
    }
}

impl ExtTrayV1RequestHandler for ExtTrayV1 {
    type Error = ExtTrayManagerV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn get_tray_item(&self, req: GetTrayItem, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let surface = self.client.lookup(req.surface)?;
        let fs = Rc::new(ExtTrayItemV1::new(
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
    self = ExtTrayV1;
    version = self.version;
}

impl Object for ExtTrayV1 {}

simple_add_obj!(ExtTrayV1);

#[derive(Debug, Error)]
pub enum ExtTrayManagerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error(transparent)]
    ExtTrayItemV1Error(#[from] ExtTrayItemV1Error),
}
efrom!(ExtTrayManagerV1Error, ClientError);
