use crate::backend::Backend;
use std::rc::Rc;

pub struct DummyBackend {}

impl DummyBackend {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {})
    }
}

impl Backend for DummyBackend {}
