use {
    crate::{
        backend::InputDevice,
        ifs::wl_seat::PX_PER_SCROLL,
        state::{DeviceHandlerData, InputDeviceData, State},
        tasks::udev_utils::{udev_props, UdevProps},
        utils::asyncevent::AsyncEvent,
    },
    jay_config::_private::DEFAULT_SEAT_NAME,
    std::{cell::Cell, rc::Rc},
};

pub fn handle(state: &Rc<State>, dev: Rc<dyn InputDevice>) {
    let props = match dev.dev_t() {
        None => UdevProps::default(),
        Some(dev_t) => udev_props(dev_t, 3),
    };
    let data = Rc::new(DeviceHandlerData {
        seat: Default::default(),
        px_per_scroll_wheel: Cell::new(PX_PER_SCROLL),
        device: dev.clone(),
        syspath: props.syspath,
        devnode: props.devnode,
        keymap: Default::default(),
        xkb_state: Default::default(),
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
        for seat in self.state.globals.seats.lock().values() {
            if seat.seat_name() == DEFAULT_SEAT_NAME {
                self.data.set_seat(Some(seat.clone()));
                break;
            }
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
                    seat.event(&self.data, event);
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
