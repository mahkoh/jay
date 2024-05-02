use {
    crate::{
        backend::DrmDeviceId,
        client::{Client, ClientCaps, ClientError, CAP_DRM_LEASE},
        globals::{Global, GlobalName},
        ifs::{
            wp_drm_lease_connector_v1::WpDrmLeaseConnectorV1,
            wp_drm_lease_request_v1::WpDrmLeaseRequestV1,
        },
        leaks::Tracker,
        object::{Object, Version},
        state::OutputData,
        utils::{bindings::Bindings, errorfmt::ErrorFmt, oserror::OsError},
        video::drm::{Drm, DrmError},
        wire::{wp_drm_lease_device_v1::*, WpDrmLeaseDeviceV1Id},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
    uapi::{c, OwnedFd},
};

mod removed_device;

pub struct WpDrmLeaseDeviceV1Global {
    pub name: GlobalName,
    pub device: DrmDeviceId,
    pub bindings: Rc<Bindings<WpDrmLeaseDeviceV1>>,
}

impl WpDrmLeaseDeviceV1Global {
    fn bind_(
        self: Rc<Self>,
        id: WpDrmLeaseDeviceV1Id,
        client: &Rc<Client>,
        version: Version,
    ) -> Result<(), WpDrmLeaseDeviceV1Error> {
        let obj = Rc::new(WpDrmLeaseDeviceV1 {
            id,
            client: client.clone(),
            tracker: Default::default(),
            version,
            bindings: self.bindings.clone(),
            device: self.device,
            destroyed: Cell::new(false),
        });
        track!(client, obj);
        client.add_client_obj(&obj)?;
        if let Some(dev) = client.state.drm_devs.get(&self.device) {
            if let Some(node) = &dev.devnode {
                match reopen_card(node) {
                    Ok(f) => obj.send_drm_fd(&f),
                    Err(e) => {
                        log::error!("Could not open master device: {}", ErrorFmt(e));
                    }
                }
            }
            for c in dev.connectors.lock().keys() {
                if let Some(o) = client.state.outputs.get(c) {
                    if o.monitor_info.non_desktop {
                        obj.create_connector(&o);
                    }
                }
            }
        }
        obj.send_done();
        self.bindings.add(client, &obj);
        Ok(())
    }
}

global_base!(
    WpDrmLeaseDeviceV1Global,
    WpDrmLeaseDeviceV1,
    WpDrmLeaseDeviceV1Error
);

simple_add_global!(WpDrmLeaseDeviceV1Global);

impl Global for WpDrmLeaseDeviceV1Global {
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

pub struct WpDrmLeaseDeviceV1 {
    pub id: WpDrmLeaseDeviceV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub bindings: Rc<Bindings<Self>>,
    pub device: DrmDeviceId,
    pub destroyed: Cell<bool>,
}

impl WpDrmLeaseDeviceV1 {
    fn detach(&self) {
        self.destroyed.set(true);
        self.bindings.remove(&self.client, self);
    }

    pub fn create_connector(self: &Rc<Self>, output: &Rc<OutputData>) {
        let id = match self.client.new_id() {
            Ok(i) => i,
            Err(e) => {
                self.client.error(e);
                return;
            }
        };
        let obj = Rc::new(WpDrmLeaseConnectorV1 {
            id,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            device: self.clone(),
            connector_id: output.connector.connector.id(),
            bindings: output.lease_connectors.clone(),
        });
        self.client.add_server_obj(&obj);
        self.send_connector(&obj);
        obj.send_name(&output.connector.name);
        if let Some(id) = output.connector.connector.drm_object_id() {
            obj.send_connector_id(id.0);
        }
        obj.send_done();
        output.lease_connectors.add(&self.client, &obj);
    }

    fn send_drm_fd(&self, fd: &Rc<OwnedFd>) {
        self.client.event(DrmFd {
            self_id: self.id,
            fd: fd.clone(),
        });
    }

    fn send_connector(&self, c: &Rc<WpDrmLeaseConnectorV1>) {
        self.client.event(Connector {
            self_id: self.id,
            id: c.id,
        });
    }

    pub fn send_done(&self) {
        self.client.event(Done { self_id: self.id });
    }

    fn send_released(&self) {
        self.client.event(Released { self_id: self.id });
    }
}

impl WpDrmLeaseDeviceV1RequestHandler for WpDrmLeaseDeviceV1 {
    type Error = WpDrmLeaseDeviceV1Error;

    fn create_lease_request(
        &self,
        req: CreateLeaseRequest,
        _slf: &Rc<Self>,
    ) -> Result<(), Self::Error> {
        let obj = Rc::new(WpDrmLeaseRequestV1 {
            id: req.id,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
            device: self.device,
            connectors: Default::default(),
        });
        self.client.add_client_obj(&obj)?;
        Ok(())
    }

    fn release(&self, _req: Release, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.detach();
        self.send_released();
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = WpDrmLeaseDeviceV1;
    version = self.version;
}

impl Object for WpDrmLeaseDeviceV1 {
    fn break_loops(&self) {
        self.detach();
    }
}

simple_add_obj!(WpDrmLeaseDeviceV1);

#[derive(Debug, Error)]
pub enum WpDrmLeaseDeviceV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(WpDrmLeaseDeviceV1Error, ClientError);

#[derive(Debug, Error)]
enum ReopenError {
    #[error("Could not open the dev node")]
    OpenNode(#[source] OsError),
    #[error("Could not drop DRM master")]
    DropMaster(#[source] DrmError),
}

fn reopen_card(devnode: &str) -> Result<Rc<OwnedFd>, ReopenError> {
    let fd = uapi::open(devnode, c::O_RDWR | c::O_CLOEXEC, 0)
        .map_err(|e| ReopenError::OpenNode(e.into()))?;
    let fd = Rc::new(fd);
    let drm = Drm::open_existing(fd.clone());
    if drm.is_master() {
        drm.drop_master().map_err(ReopenError::DropMaster)?;
    }
    Ok(fd)
}
