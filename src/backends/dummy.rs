use std::rc::Rc;
use crate::backend::Backend;

pub struct DummyBackend {

}

impl DummyBackend {
    pub fn new() -> Rc<Self> {
        Rc::new(Self { })
    }
}

impl Backend for DummyBackend {
}
