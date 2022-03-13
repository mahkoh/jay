use crate::fixed::Fixed;
use std::fmt::Debug;
use std::rc::Rc;

linear_ids!(OutputIds, OutputId);
linear_ids!(InputDeviceIds, InputDeviceId);

pub trait Backend {
    fn switch_to(&self, vtnr: u32);
}

pub trait Output {
    fn id(&self) -> OutputId;
    fn removed(&self) -> bool;
    fn width(&self) -> i32;
    fn height(&self) -> i32;
    fn on_change(&self, cb: Rc<dyn Fn()>);
}

pub trait InputDevice {
    fn id(&self) -> InputDeviceId;
    fn removed(&self) -> bool;
    fn event(&self) -> Option<InputEvent>;
    fn on_change(&self, cb: Rc<dyn Fn()>);
    fn grab(&self, grab: bool);
}

pub enum BackendEvent {
    NewOutput(Rc<dyn Output>),
    NewInputDevice(Rc<dyn InputDevice>),
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
pub enum InputEvent {
    Key(u32, KeyState),
    OutputPosition(OutputId, Fixed, Fixed),
    #[allow(dead_code)]
    Motion(Fixed, Fixed),
    Button(u32, KeyState),
    Scroll(i32, ScrollAxis),
}
