use {
    crate::{
        backend::{ConnectorId, DrmDeviceId},
        client::{Client, ClientError},
        ifs::wp_drm_lease_v1::{WpDrmLeaseV1, WpDrmLeaseV1Lessee},
        leaks::Tracker,
        object::{Object, Version},
        utils::copyhashmap::CopyHashMap,
        wire::{wp_drm_lease_request_v1::*, WpDrmLeaseConnectorV1Id, WpDrmLeaseRequestV1Id},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct WpDrmLeaseRequestV1 {
    pub id: WpDrmLeaseRequestV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub device: DrmDeviceId,
    pub connectors: CopyHashMap<WpDrmLeaseConnectorV1Id, ConnectorId>,
}

impl WpDrmLeaseRequestV1RequestHandler for WpDrmLeaseRequestV1 {
    type Error = WpDrmLeaseRequestV1Error;

    fn request_connector(&self, req: RequestConnector, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let c = self.client.lookup(req.connector)?;
        if self.device != c.device.device {
            return Err(WpDrmLeaseRequestV1Error::MismatchedDevice(c.id));
        }
        if self.connectors.contains(&c.id) {
            return Err(WpDrmLeaseRequestV1Error::RepeatedDevice(c.id));
        }
        self.connectors.set(c.id, c.connector_id);
        Ok(())
    }

    fn submit(&self, req: Submit, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        let obj = Rc::new(WpDrmLeaseV1 {
            id: req.id,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            finished: Cell::new(false),
            lease: Default::default(),
        });
        track!(self.client, obj);
        self.client.add_client_obj(&obj)?;
        if self.connectors.is_empty() {
            return Err(WpDrmLeaseRequestV1Error::EmptyLease);
        }
        let Some(dev) = self.client.state.drm_devs.get(&self.device) else {
            obj.send_finished();
            return Ok(());
        };
        let lessee = Rc::new(WpDrmLeaseV1Lessee { obj });
        let connectors: Vec<_> = self.connectors.lock().values().copied().collect();
        dev.dev.clone().create_lease(lessee, &connectors);
        Ok(())
    }
}

object_base! {
    self = WpDrmLeaseRequestV1;
    version = self.version;
}

impl Object for WpDrmLeaseRequestV1 {}

simple_add_obj!(WpDrmLeaseRequestV1);

#[derive(Debug, Error)]
pub enum WpDrmLeaseRequestV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Connector {0} does not belong to this device")]
    MismatchedDevice(WpDrmLeaseConnectorV1Id),
    #[error("Connector {0} is already part of this request")]
    RepeatedDevice(WpDrmLeaseConnectorV1Id),
    #[error("Lease request is empty")]
    EmptyLease,
}
efrom!(WpDrmLeaseRequestV1Error, ClientError);
