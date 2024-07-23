use {crate::ei::ei_object::EiVersion, std::cell::Cell};

pub mod ei_acceptor;
pub mod ei_client;
pub mod ei_ifs;
pub mod ei_object;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum EiContext {
    Sender,
    Receiver,
}

pub struct EiInterfaceVersion {
    pub server_max_version: EiVersion,
    pub client_max_version: Cell<EiVersion>,
    pub version: Cell<EiVersion>,
}

impl EiInterfaceVersion {
    pub fn new(server_max_version: u32) -> Self {
        Self {
            server_max_version: EiVersion(server_max_version),
            client_max_version: Cell::new(EiVersion(0)),
            version: Cell::new(EiVersion(0)),
        }
    }

    pub fn set_client_version(&self, version: EiVersion) {
        self.client_max_version.set(version);
        self.version.set(self.server_max_version.min(version));
    }
}
