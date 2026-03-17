use {
    crate::{
        async_engine::SpawnedFuture,
        backend::{
            self, Backend, BackendConnectorState, BackendConnectorStateSerial, Connector,
            ConnectorEvent, ConnectorId, ConnectorKernelId, DrmDeviceId,
        },
        format::XRGB8888,
        video::drm::ConnectorType,
    },
    std::{error::Error, rc::Rc},
};

pub struct DummyBackend;

impl Backend for DummyBackend {
    fn run(self: Rc<Self>) -> SpawnedFuture<Result<(), Box<dyn Error>>> {
        unreachable!();
    }
}

pub struct DummyOutput {
    pub id: ConnectorId,
}

impl Connector for DummyOutput {
    fn id(&self) -> ConnectorId {
        self.id
    }

    fn kernel_id(&self) -> ConnectorKernelId {
        ConnectorKernelId {
            ty: ConnectorType::Unknown(0),
            idx: 0,
        }
    }

    fn event(&self) -> Option<ConnectorEvent> {
        None
    }

    fn on_change(&self, _cb: Rc<dyn Fn()>) {
        // nothing
    }

    fn damage(&self) {
        // nothing
    }

    fn drm_dev(&self) -> Option<DrmDeviceId> {
        None
    }

    fn effectively_locked(&self) -> bool {
        true
    }

    fn state(&self) -> BackendConnectorState {
        let mode = backend::Mode {
            width: 0,
            height: 0,
            refresh_rate_millihz: 40_000,
        };
        BackendConnectorState {
            serial: BackendConnectorStateSerial::from_raw(0),
            enabled: true,
            active: false,
            mode,
            non_desktop_override: None,
            vrr: false,
            tearing: false,
            format: XRGB8888,
            color_space: Default::default(),
            eotf: Default::default(),
            gamma_lut: Default::default(),
        }
    }
}
