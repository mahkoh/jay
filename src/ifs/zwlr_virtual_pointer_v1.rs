use {
    crate::{
        backend::{AxisSource as BackendAxisSource, ButtonState, ScrollAxis},
        client::{Client, ClientError},
        fixed::Fixed,
        ifs::{
            wl_output::OutputGlobalOpt,
            wl_seat::{
                PX_PER_SCROLL, WlSeatGlobal,
                wl_pointer::{self, CONTINUOUS, FINGER, PRESSED, RELEASED, WHEEL},
            },
        },
        leaks::Tracker,
        object::{Object, Version},
        utils::{copyhashmap::CopyHashMap, syncqueue::SyncQueue},
        wire::{ZwlrVirtualPointerV1Id, zwlr_virtual_pointer_v1::*},
    },
    std::rc::Rc,
    thiserror::Error,
};

pub struct ZwlrVirtualPointerV1 {
    pub id: ZwlrVirtualPointerV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub events: SyncQueue<Event>,
    pub seat: Rc<WlSeatGlobal>,
    pub output: Option<Rc<OutputGlobalOpt>>,
    pub buttons: CopyHashMap<u32, ()>,
}

pub enum Event {
    Motion(u32, Fixed, Fixed),
    MotionAbsolute(u32, u32, u32, u32, u32),
    Button(u32, u32, ButtonState),
    Axis(u32, ScrollAxis, Fixed),
    AxisSource(BackendAxisSource),
    AxisStop(u32, ScrollAxis),
    AxisDiscrete(u32, ScrollAxis, Fixed, i32),
}

fn map_axis(axis: u32) -> Result<ScrollAxis, ZwlrVirtualPointerV1Error> {
    const VERTICAL_SCROLL: u32 = wl_pointer::VERTICAL_SCROLL as u32;
    const HORIZONTAL_SCROLL: u32 = wl_pointer::HORIZONTAL_SCROLL as u32;
    let axis = match axis {
        VERTICAL_SCROLL => ScrollAxis::Vertical,
        HORIZONTAL_SCROLL => ScrollAxis::Horizontal,
        n => return Err(ZwlrVirtualPointerV1Error::UnknownAxis(n)),
    };
    Ok(axis)
}

impl ZwlrVirtualPointerV1 {
    fn detach(&self) {
        for (button, _) in self.buttons.lock().drain() {
            let now = self.client.state.now_usec();
            self.seat.button_event(now, button, ButtonState::Released);
        }
    }
}

impl ZwlrVirtualPointerV1RequestHandler for ZwlrVirtualPointerV1 {
    type Error = ZwlrVirtualPointerV1Error;

    fn motion(&self, req: Motion, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.events.push(Event::Motion(req.time, req.dx, req.dy));
        Ok(())
    }

    fn motion_absolute(&self, req: MotionAbsolute, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.events.push(Event::MotionAbsolute(
            req.time,
            req.x,
            req.y,
            req.x_extent,
            req.y_extent,
        ));
        Ok(())
    }

    fn button(&self, req: Button, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let state = match req.state {
            RELEASED => ButtonState::Released,
            PRESSED => ButtonState::Pressed,
            n => return Err(ZwlrVirtualPointerV1Error::UnknownButtonState(n)),
        };
        self.events.push(Event::Button(req.time, req.button, state));
        Ok(())
    }

    fn axis(&self, req: Axis, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.events
            .push(Event::Axis(req.time, map_axis(req.axis)?, req.value));
        Ok(())
    }

    fn frame(&self, _req: Frame, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        fn ms_to_us(ms: u32) -> u64 {
            ms as u64 * 1_000
        }
        let mut axis_time = None;
        while let Some(ev) = self.events.pop() {
            match ev {
                Event::Motion(time, dx, dy) => {
                    self.seat.motion_event(ms_to_us(time), dx, dy, dx, dy);
                }
                Event::MotionAbsolute(time, x, y, x_max, y_max) => {
                    let x = x as f32 / x_max as f32;
                    let y = y as f32 / y_max as f32;
                    let rect = self
                        .output
                        .as_ref()
                        .and_then(|c| c.get())
                        .map(|g| g.pos.get())
                        .unwrap_or_else(|| self.client.state.root.extents.get());
                    self.seat.motion_absolute_event(ms_to_us(time), rect, x, y);
                }
                Event::Button(time, button, state) => {
                    match state {
                        ButtonState::Released => self.buttons.remove(&button),
                        ButtonState::Pressed => self.buttons.set(button, ()),
                    };
                    self.seat.button_event(ms_to_us(time), button, state);
                }
                Event::Axis(time, axis, v) => {
                    axis_time = Some(time);
                    self.seat.axis_px(v, axis, false);
                }
                Event::AxisSource(source) => {
                    self.seat.axis_source(source);
                }
                Event::AxisStop(time, axis) => {
                    axis_time = Some(time);
                    self.seat.axis_stop(axis);
                }
                Event::AxisDiscrete(time, axis, value, discrete) => {
                    axis_time = Some(time);
                    self.seat.axis_px(value, axis, false);
                    self.seat
                        .axis_120(discrete.saturating_mul(120), axis, false);
                }
            }
        }
        if let Some(time) = axis_time {
            self.seat.axis_frame(PX_PER_SCROLL, ms_to_us(time));
        }
        Ok(())
    }

    fn axis_source(&self, req: AxisSource, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let source = match req.axis_source {
            WHEEL => BackendAxisSource::Wheel,
            FINGER => BackendAxisSource::Finger,
            CONTINUOUS => BackendAxisSource::Continuous,
            n => return Err(ZwlrVirtualPointerV1Error::UnknownAxisSource(n)),
        };
        self.events.push(Event::AxisSource(source));
        Ok(())
    }

    fn axis_stop(&self, req: AxisStop, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.events
            .push(Event::AxisStop(req.time, map_axis(req.axis)?));
        Ok(())
    }

    fn axis_discrete(&self, req: AxisDiscrete, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.events.push(Event::AxisDiscrete(
            req.time,
            map_axis(req.axis)?,
            req.value,
            req.discrete,
        ));
        Ok(())
    }

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        self.detach();
        Ok(())
    }
}

object_base! {
    self = ZwlrVirtualPointerV1;
    version = self.version;
}

impl Object for ZwlrVirtualPointerV1 {
    fn break_loops(&self) {
        self.detach();
    }
}

simple_add_obj!(ZwlrVirtualPointerV1);

#[derive(Debug, Error)]
pub enum ZwlrVirtualPointerV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
    #[error("Unknown button state {0}")]
    UnknownButtonState(u32),
    #[error("Unknown axis {0}")]
    UnknownAxis(u32),
    #[error("Unknown axis source {0}")]
    UnknownAxisSource(u32),
}
efrom!(ZwlrVirtualPointerV1Error, ClientError);
