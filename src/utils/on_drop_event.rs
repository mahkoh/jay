use {crate::utils::asyncevent::AsyncEvent, std::rc::Rc};

#[derive(Default)]
pub struct OnDropEvent {
    ae: Rc<AsyncEvent>,
}

impl OnDropEvent {
    pub fn event(&self) -> Rc<AsyncEvent> {
        self.ae.clone()
    }
}

impl Drop for OnDropEvent {
    fn drop(&mut self) {
        self.ae.trigger()
    }
}
