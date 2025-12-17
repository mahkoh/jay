use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        leaks::Tracker,
        object::{Object, Version},
        wire::{WlUpgradeId, wl_upgrade::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WlUpgradeGlobal {
    pub name: GlobalName,
}

impl WlUpgradeGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WlUpgradeId,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), WlUpgradeError> {
        let mgr = Rc::new(WlUpgrade {
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

global_base!(WlUpgradeGlobal, WlUpgrade, WlUpgradeError);

simple_add_global!(WlUpgradeGlobal);

impl Global for WlUpgradeGlobal {
    fn singleton(&self) -> bool {
        true
    }

    fn version(&self) -> u32 {
        1
    }
}

pub struct WlUpgrade {
    pub id: WlUpgradeId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl WlUpgradeRequestHandler for WlUpgrade {
    type Error = WlUpgradeError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }

    fn upgrade(&self, _req: Upgrade, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.event(Upgraded { self_id: self.id });
        self.client.v2.set(true);
        Ok(())
    }
}

object_base! {
    self = WlUpgrade;
    version = self.version;
}

impl Object for WlUpgrade {}

simple_add_obj!(WlUpgrade);

#[derive(Debug, Error)]
pub enum WlUpgradeError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WlUpgradeError, ClientError);
