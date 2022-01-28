use crate::fixed::Fixed;
use std::rc::Rc;

linear_ids!(OutputIds, OutputId);
linear_ids!(SeatIds, SeatId);

pub trait Output {
    fn id(&self) -> OutputId;
    fn removed(&self) -> bool;
    fn width(&self) -> i32;
    fn height(&self) -> i32;
    fn on_change(&self, cb: Rc<dyn Fn()>);
}

pub trait Seat {
    fn id(&self) -> SeatId;
    fn removed(&self) -> bool;
    fn event(&self) -> Option<SeatEvent>;
    fn on_change(&self, cb: Rc<dyn Fn()>);
}

pub enum BackendEvent {
    NewOutput(Rc<dyn Output>),
    NewSeat(Rc<dyn Seat>),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum KeyState {
    Released,
    Pressed,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ScrollAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug)]
pub enum SeatEvent {
    OutputPosition(OutputId, Fixed, Fixed),
    #[allow(dead_code)]
    Motion(Fixed, Fixed),
    Button(u32, KeyState),
    Scroll(i32, ScrollAxis),
    Key(u32, KeyState),
}
