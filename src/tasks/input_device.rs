use {
    crate::{
        backend::InputDevice,
        state::{DeviceHandlerData, InputDeviceData, State},
        utils::asyncevent::AsyncEvent,
    },
    std::rc::Rc,
};

pub fn handle(state: &Rc<State>, dev: Rc<dyn InputDevice>) {
    let data = Rc::new(DeviceHandlerData {
        seat: Default::default(),
        device: dev.clone(),
    });
    let ae = Rc::new(AsyncEvent::default());
    let oh = DeviceHandler {
        state: state.clone(),
        dev: dev.clone(),
        data: data.clone(),
        ae: ae.clone(),
    };
    let handler = state.eng.spawn(oh.handle());
    state.input_device_handlers.borrow_mut().insert(
        dev.id(),
        InputDeviceData {
            handler,
            id: dev.id(),
            data,
            async_event: ae,
        },
    );
}

struct DeviceHandler {
    state: Rc<State>,
    dev: Rc<dyn InputDevice>,
    data: Rc<DeviceHandlerData>,
    ae: Rc<AsyncEvent>,
}

impl DeviceHandler {
    pub async fn handle(self) {
        {
            let ae = self.ae.clone();
            self.dev.on_change(Rc::new(move || ae.trigger()));
        }
        if let Some(config) = self.state.config.get() {
            config.new_input_device(self.dev.id());
        }
        loop {
            if self.dev.removed() {
                break;
            }
            if let Some(seat) = self.data.seat.get() {
                let mut any_events = false;
                while let Some(event) = self.dev.event() {
                    seat.event(event);
                    any_events = true;
                }
                if any_events {
                    seat.mark_last_active();
                    self.state.input_occurred();
                }
            } else {
                while self.dev.event().is_some() {
                    // nothing
                }
            }
            self.ae.triggered().await;
        }
        if let Some(config) = self.state.config.get() {
            config.del_input_device(self.dev.id());
        }
        self.state
            .input_device_handlers
            .borrow_mut()
            .remove(&self.dev.id());
    }
}
