use crate::async_engine::SpawnedFuture;
use crate::backend::{Keyboard, KeyboardEvent, Mouse, MouseEvent};
use crate::config::ConfigProxy;
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::state::{DeviceHandlerData, KeyboardData, MouseData};
use crate::utils::asyncevent::AsyncEvent;
use crate::State;
use std::rc::Rc;

pub trait DeviceApi: 'static {
    type Event;

    fn on_change(&self, cb: Rc<dyn Fn()>);
    fn announce(&self, config: &ConfigProxy);
    fn announce_del(&self, config: &ConfigProxy);
    fn removed(&self) -> bool;
    fn add(self: &Rc<Self>, state: &State, handler: SpawnedFuture<()>, data: Rc<DeviceHandlerData>);
    fn remove(&self, state: &State);
    fn event(&self) -> Option<Self::Event>;
    fn send(seat: &Rc<WlSeatGlobal>, event: Self::Event);
}

impl DeviceApi for dyn Keyboard {
    type Event = KeyboardEvent;

    fn on_change(&self, cb: Rc<dyn Fn()>) {
        self.on_change(cb);
    }

    fn announce(&self, config: &ConfigProxy) {
        config.new_keyboard(self.id());
    }

    fn announce_del(&self, config: &ConfigProxy) {
        config.del_keyboard(self.id());
    }

    fn removed(&self) -> bool {
        self.removed()
    }

    fn add(
        self: &Rc<Self>,
        state: &State,
        handler: SpawnedFuture<()>,
        data: Rc<DeviceHandlerData>,
    ) {
        state.kb_handlers.borrow_mut().insert(
            self.id(),
            KeyboardData {
                handler,
                id: self.id(),
                kb: self.clone(),
                data,
            },
        );
    }

    fn remove(&self, state: &State) {
        state.kb_handlers.borrow_mut().remove(&self.id());
    }

    fn event(&self) -> Option<Self::Event> {
        self.event()
    }

    fn send(seat: &Rc<WlSeatGlobal>, event: Self::Event) {
        seat.kb_event(event);
    }
}

impl DeviceApi for dyn Mouse {
    type Event = MouseEvent;

    fn on_change(&self, cb: Rc<dyn Fn()>) {
        self.on_change(cb);
    }

    fn announce(&self, config: &ConfigProxy) {
        config.new_mouse(self.id());
    }

    fn announce_del(&self, config: &ConfigProxy) {
        config.del_mouse(self.id());
    }

    fn removed(&self) -> bool {
        self.removed()
    }

    fn add(
        self: &Rc<Self>,
        state: &State,
        handler: SpawnedFuture<()>,
        data: Rc<DeviceHandlerData>,
    ) {
        state.mouse_handlers.borrow_mut().insert(
            self.id(),
            MouseData {
                handler,
                id: self.id(),
                data,
            },
        );
    }

    fn remove(&self, state: &State) {
        state.mouse_handlers.borrow_mut().remove(&self.id());
    }

    fn event(&self) -> Option<Self::Event> {
        self.event()
    }

    fn send(seat: &Rc<WlSeatGlobal>, event: Self::Event) {
        seat.mouse_event(event);
    }
}

pub fn handle<T: DeviceApi + ?Sized>(state: &Rc<State>, dev: Rc<T>) {
    let data = Rc::new(DeviceHandlerData {
        seat: Default::default(),
    });
    let oh = DeviceHandler {
        state: state.clone(),
        dev: dev.clone(),
        data: data.clone(),
    };
    let handler = state.eng.spawn(oh.handle());
    dev.add(&state, handler, data);
}

pub struct DeviceHandler<T: DeviceApi + ?Sized> {
    pub state: Rc<State>,
    pub dev: Rc<T>,
    pub data: Rc<DeviceHandlerData>,
}

impl<T: DeviceApi + ?Sized> DeviceHandler<T> {
    pub async fn handle(self) {
        let ae = Rc::new(AsyncEvent::default());
        {
            let ae = ae.clone();
            self.dev.on_change(Rc::new(move || ae.trigger()));
        }
        if let Some(config) = self.state.config.get() {
            self.dev.announce(&config);
        }
        loop {
            if self.dev.removed() {
                break;
            }
            if let Some(seat) = self.data.seat.get() {
                let mut any_events = false;
                while let Some(event) = self.dev.event() {
                    T::send(&seat, event);
                    any_events = true;
                }
                if any_events {
                    seat.mark_last_active();
                }
            } else {
                while self.dev.event().is_some() {
                    // nothing
                }
            }
            ae.triggered().await;
        }
        if let Some(config) = self.state.config.get() {
            self.dev.announce_del(&config);
        }
        self.dev.remove(&self.state);
    }
}
