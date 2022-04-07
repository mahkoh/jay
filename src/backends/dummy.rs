use {
    crate::{
        backend::{Backend, Connector, ConnectorEvent, ConnectorId, ConnectorKernelId},
        video::drm::ConnectorType,
    },
    std::rc::Rc,
};

pub struct DummyBackend {}

impl Backend for DummyBackend {
    fn switch_to(&self, vtnr: u32) {
        let _ = vtnr;
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
}
