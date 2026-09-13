use crate::backend::DrmDeviceId;
use crate::client::CAP_DRM_LEASE;
use crate::client::Client;
use crate::client::ClientCaps;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::globals::RemovableWaylandGlobal;
use crate::ifs::wp_drm_lease_device_v1::WpDrmLeaseDeviceV1;
use crate::ifs::wp_drm_lease_device_v1::WpDrmLeaseDeviceV1Global;
use crate::object::Version;
use crate::utils::bindings::Bindings;
use crate::wire::WpDrmLeaseDeviceV1Id;
use std::cell::Cell;
use std::convert::Infallible;
use std::rc::Rc;

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
    ) -> Result<(), Infallible> {
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
        client.add_client_obj(&dev);
        dev.send_done();
        dev.bindings.add(client, &dev);
        Ok(())
    }
}

global_base!(RemovedWpDrmLeaseDeviceV1Global, WpDrmLeaseDeviceV1);

simple_add_global!(RemovedWpDrmLeaseDeviceV1Global);

impl Global for RemovedWpDrmLeaseDeviceV1Global {
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
