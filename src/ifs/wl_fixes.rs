use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        leaks::Tracker,
        object::{Object, Version},
        wire::{wl_fixes::*, WlFixesId},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WlFixesGlobal {
    pub name: GlobalName,
}

impl WlFixesGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WlFixesId,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), WlFixesError> {
        let mgr = Rc::new(WlFixes {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, mgr);
        client.add_client_obj(&mgr)?;
        Ok(())
    }
}

global_base!(WlFixesGlobal, WlFixes, WlFixesError);

simple_add_global!(WlFixesGlobal);

impl Global for WlFixesGlobal {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

pub struct WlFixes {
    pub id: WlFixesId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl WlFixesRequestHandler for WlFixes {
    type Error = WlFixesError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn destroy_registry(&self, req: DestroyRegistry, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let registry = self.client.lookup(req.registry)?;
        self.client.remove_obj(&*registry)?;
        Ok(())
    }
}

object_base! {
    self = WlFixes;
    version = self.version;
}

impl Object for WlFixes {}

simple_add_obj!(WlFixes);

#[derive(Debug, Error)]
pub enum WlFixesError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WlFixesError, ClientError);
