use crate::backend::{BackendEvent, Output, Seat};
use crate::ifs::wl_output::WlOutputGlobal;
use crate::ifs::wl_seat::WlSeatGlobal;
use crate::tree::{NodeCommon, NodeExtents, OutputNode};
use crate::utils::asyncevent::AsyncEvent;
use crate::State;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub async fn handle_backend_events(state: Rc<State>) {
    let mut beh = BackendEventHandler { state };
    beh.handle_events().await;
}

struct BackendEventHandler {
    state: Rc<State>,
}

impl BackendEventHandler {
    async fn handle_events(&mut self) {
        loop {
            let event = self.state.backend_events.pop().await;
            self.handle_event(event).await;
        }
    }

    async fn handle_event(&mut self, event: BackendEvent) {
        match event {
            BackendEvent::NewOutput(output) => self.handle_new_output(output).await,
            BackendEvent::NewSeat(seat) => self.handle_new_seat(seat).await,
        }
    }

    async fn handle_new_output(&mut self, output: Rc<dyn Output>) {
        let id = output.id();
        let oh = OutputHandler {
            state: self.state.clone(),
            output,
        };
        let future = self.state.eng.spawn(oh.handle());
        self.state.output_handlers.borrow_mut().insert(id, future);
    }

    async fn handle_new_seat(&mut self, seat: Rc<dyn Seat>) {
        let id = seat.id();
        let oh = SeatHandler {
            state: self.state.clone(),
            seat,
        };
        let future = self.state.eng.spawn(oh.handle());
        self.state.seat_handlers.borrow_mut().insert(id, future);
    }
}

struct OutputHandler {
    state: Rc<State>,
    output: Rc<dyn Output>,
}

impl OutputHandler {
    async fn handle(self) {
        let ae = Rc::new(AsyncEvent::default());
        {
            let ae = ae.clone();
            self.output.on_change(Rc::new(move || ae.trigger()));
        }
        let on = Rc::new(OutputNode {
            common: NodeCommon {
                extents: Cell::new(NodeExtents {
                    x: 0,
                    y: 0,
                    width: self.output.width(),
                    height: self.output.height(),
                }),
                id: self.state.node_ids.next(),
                parent: Some(self.state.root.clone()),
                floating_outputs: RefCell::new(Default::default()),
            },
            backend: self.output.clone(),
            child: RefCell::new(None),
            floating: Default::default(),
        });
        self.state.root.outputs.set(self.output.id(), on.clone());
        let name = self.state.globals.name();
        let global = Rc::new(WlOutputGlobal::new(name, &self.output));
        self.state.add_global(&global).await;
        self.state.outputs.set(self.output.id(), global.clone());
        loop {
            if self.output.removed() {
                break;
            }
            on.common.extents.set(NodeExtents {
                x: 0,
                y: 0,
                width: self.output.width(),
                height: self.output.height(),
            });
            global.update_properties().await;
            ae.triggered().await;
        }
        self.state.outputs.remove(&self.output.id());
        self.state.globals.remove(&self.state, name).await;
        self.state
            .output_handlers
            .borrow_mut()
            .remove(&self.output.id());
    }
}

struct SeatHandler {
    state: Rc<State>,
    seat: Rc<dyn Seat>,
}

impl SeatHandler {
    async fn handle(self) {
        let ae = Rc::new(AsyncEvent::default());
        {
            let ae = ae.clone();
            self.seat.on_change(Rc::new(move || ae.trigger()));
        }
        let name = self.state.globals.name();
        let global = Rc::new(WlSeatGlobal::new(name, &self.state, &self.seat));
        self.state.add_global(&global).await;
        loop {
            if self.seat.removed() {
                break;
            }
            while let Some(event) = self.seat.event() {
                global.event(event).await;
            }
            ae.triggered().await;
        }
        self.state.globals.remove(&self.state, name).await;
        self.state
            .seat_handlers
            .borrow_mut()
            .remove(&self.seat.id());
    }
}
