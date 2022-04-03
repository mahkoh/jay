use crate::backend::{Backend, Connector, ConnectorEvent, OutputId};
use std::rc::Rc;

pub struct DummyBackend {}

impl Backend for DummyBackend {
    fn switch_to(&self, vtnr: u32) {
        let _ = vtnr;
    }
}

pub struct DummyOutput {
    pub id: OutputId,
}

impl Connector for DummyOutput {
    fn id(&self) -> OutputId {
        self.id
    }

    fn event(&self) -> Option<ConnectorEvent> {
        None
    }

    fn on_change(&self, _cb: Rc<dyn Fn()>) {
        // nothing
    }
}
