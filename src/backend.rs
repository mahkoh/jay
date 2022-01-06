use std::rc::Rc;
use crate::fixed::Fixed;

linear_ids!(OutputIds, OutputId);
linear_ids!(SeatIds, SeatId);

pub trait Output {
    fn id(&self) -> OutputId;
    fn removed(&self) -> bool;
    fn width(&self) -> u32;
    fn height(&self) -> u32;
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

#[derive(Debug)]
pub enum SeatEvent {
    Motion(OutputId, Fixed, Fixed),
}
