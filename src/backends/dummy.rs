use crate::backend::{Backend, Output, OutputId};
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

impl Output for DummyOutput {
    fn id(&self) -> OutputId {
        self.id
    }

    fn removed(&self) -> bool {
        false
    }

    fn width(&self) -> i32 {
        100
    }

    fn height(&self) -> i32 {
        100
    }

    fn on_change(&self, _cb: Rc<dyn Fn()>) {
        // nothing
    }
}
