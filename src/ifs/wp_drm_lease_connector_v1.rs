use {
    crate::{
        backend::ConnectorId as BackendConnectorId,
        client::{Client, ClientError},
        ifs::wp_drm_lease_device_v1::WpDrmLeaseDeviceV1,
        leaks::Tracker,
        object::{Object, Version},
        utils::bindings::Bindings,
        wire::{WpDrmLeaseConnectorV1Id, wp_drm_lease_connector_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct WpDrmLeaseConnectorV1 {
    pub id: WpDrmLeaseConnectorV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub device: Rc<WpDrmLeaseDeviceV1>,
    pub connector_id: BackendConnectorId,
    pub bindings: Rc<Bindings<Self>>,
}

impl WpDrmLeaseConnectorV1 {
    fn detach(&self) {
        self.bindings.remove(&self.client, self);
    }

    pub fn send_name(&self, name: &str) {
        self.client.event(Name {
            self_id: self.id,
            name,
        });
    }

    #[expect(dead_code)]
    pub fn send_description(&self, description: &str) {
        self.client.event(Description {
            self_id: self.id,
            description,
        });
    }

    pub fn send_connector_id(&self, connector_id: u32) {
        self.client.event(ConnectorId {
            self_id: self.id,
            connector_id,
        });
    }

    pub fn send_done(&self) {
        self.client.event(Done { self_id: self.id });
    }

    pub fn send_withdrawn(&self) {
        self.client.event(Withdrawn { self_id: self.id });
    }
}

impl WpDrmLeaseConnectorV1RequestHandler for WpDrmLeaseConnectorV1 {
    type Error = WpDrmLeaseConnectorV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = WpDrmLeaseConnectorV1;
    version = self.version;
}

impl Object for WpDrmLeaseConnectorV1 {
    fn break_loops(&self) {
        self.detach();
    }
}

dedicated_add_obj!(
    WpDrmLeaseConnectorV1,
    WpDrmLeaseConnectorV1Id,
    drm_lease_outputs
);

#[derive(Debug, Error)]
pub enum WpDrmLeaseConnectorV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WpDrmLeaseConnectorV1Error, ClientError);
