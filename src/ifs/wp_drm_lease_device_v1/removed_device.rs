use {
    crate::{
        backend::DrmDeviceId,
        client::{Client, ClientCaps, ClientError, CAP_DRM_LEASE},
        globals::{Global, GlobalName, RemovableWaylandGlobal},
        ifs::wp_drm_lease_device_v1::{WpDrmLeaseDeviceV1, WpDrmLeaseDeviceV1Global},
        object::Version,
        utils::bindings::Bindings,
        wire::WpDrmLeaseDeviceV1Id,
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

struct RemovedWpDrmLeaseDeviceV1Global {
    name: GlobalName,
    bindings: Rc<Bindings<WpDrmLeaseDeviceV1>>,
}

impl RemovedWpDrmLeaseDeviceV1Global {
    fn bind_(
        self: Rc<Self>,
        id: WpDrmLeaseDeviceV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), RemovedWpDrmLeaseDeviceV1Error> {
        let dev = Rc::new(WpDrmLeaseDeviceV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
            bindings: self.bindings.clone(),
            device: DrmDeviceId::from_raw(0),
            destroyed: Cell::new(false),
        });
        track!(client, dev);
        client.add_client_obj(&dev)?;
        dev.send_done();
        dev.bindings.add(client, &dev);
        Ok(())
    }
}

global_base!(
    RemovedWpDrmLeaseDeviceV1Global,
    WpDrmLeaseDeviceV1,
    RemovedWpDrmLeaseDeviceV1Error
);

simple_add_global!(RemovedWpDrmLeaseDeviceV1Global);

impl Global for RemovedWpDrmLeaseDeviceV1Global {
    fn singleton(&self) -> bool {
        false
    }

    fn version(&self) -> u32 {
        1
    }

    fn required_caps(&self) -> ClientCaps {
        CAP_DRM_LEASE
    }
}

impl RemovableWaylandGlobal for WpDrmLeaseDeviceV1Global {
    fn create_replacement(self: Rc<Self>) -> Rc<dyn Global> {
        Rc::new(RemovedWpDrmLeaseDeviceV1Global {
            name: self.name,
            bindings: Default::default(),
        })
    }
}

#[derive(Debug, Error)]
pub enum RemovedWpDrmLeaseDeviceV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(RemovedWpDrmLeaseDeviceV1Error, ClientError);
