use {
    crate::{
        client::{Client, ClientError},
        globals::{Global, GlobalName},
        ifs::wl_display::{MAX_DISPLAY_VERSION, MIN_DISPLAY_VERSION},
        leaks::Tracker,
        object::{Object, Version},
        wire::{WlDisplayUpgradeId, wl_display_upgrade::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WlDisplayUpgradeGlobal {
    pub name: GlobalName,
}

impl WlDisplayUpgradeGlobal {
    pub fn new(name: GlobalName) -> Self {
        Self { name }
    }

    fn bind_(
        self: Rc<Self>,
        id: WlDisplayUpgradeId,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), WlDisplayUpgradeError> {
        if client.has_display_upgrade.replace(true) {
            return Err(WlDisplayUpgradeError::HasDisplayUpgrade);
        }
        let mgr = Rc::new(WlDisplayUpgrade {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
        });
        track!(client, mgr);
        client.add_client_obj(&mgr)?;
        client.event(MaxVersion {
            self_id: id,
            version: MAX_DISPLAY_VERSION.0,
        });
        Ok(())
    }
}

global_base!(
    WlDisplayUpgradeGlobal,
    WlDisplayUpgrade,
    WlDisplayUpgradeError
);

simple_add_global!(WlDisplayUpgradeGlobal);

impl Global for WlDisplayUpgradeGlobal {
    fn version(&self) -> u32 {
        1
    }
}

pub struct WlDisplayUpgrade {
    pub id: WlDisplayUpgradeId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl WlDisplayUpgradeRequestHandler for WlDisplayUpgrade {
    type Error = WlDisplayUpgradeError;

    fn upgrade(&self, req: Upgrade, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        if self.client.objects.len() > 1 {
            return Err(WlDisplayUpgradeError::HasObjects);
        }
        if req.version < MIN_DISPLAY_VERSION.0 || req.version > MAX_DISPLAY_VERSION.0 {
            return Err(WlDisplayUpgradeError::OutOfBounds);
        }
        self.client.display()?.version.set(Version(req.version));
        Ok(())
    }
}

object_base! {
    self = WlDisplayUpgrade;
    version = self.version;
}

impl Object for WlDisplayUpgrade {}

simple_add_obj!(WlDisplayUpgrade);

#[derive(Debug, Error)]
pub enum WlDisplayUpgradeError {
    #[error("Tried to bind wl_display_upgrade more than once")]
    HasDisplayUpgrade,
    #[error("Tried to upgrade with existing objects")]
    HasObjects,
    #[error("The requested version is out of bounds")]
    OutOfBounds,
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WlDisplayUpgradeError, ClientError);
