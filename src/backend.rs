use crate::fixed::Fixed;
use std::fmt::Debug;
use std::rc::Rc;

linear_ids!(OutputIds, OutputId);
linear_ids!(InputDeviceIds, InputDeviceId);

pub trait Backend {
    fn switch_to(&self, vtnr: u32);
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Mode {
    pub width: i32,
    pub height: i32,
    pub refresh_rate: u32,
}

pub trait Connector {
    fn id(&self) -> OutputId;
    fn event(&self) -> Option<ConnectorEvent>;
    fn on_change(&self, cb: Rc<dyn Fn()>);
}

#[derive(Debug)]
pub enum ConnectorEvent {
    Removed,
    ModeChanged(Mode),
}

pub trait InputDevice {
    fn id(&self) -> InputDeviceId;
    fn removed(&self) -> bool;
    fn event(&self) -> Option<InputEvent>;
    fn on_change(&self, cb: Rc<dyn Fn()>);
    fn grab(&self, grab: bool);
    fn has_capability(&self, cap: InputDeviceCapability) -> bool;
    fn set_left_handed(&self, left_handed: bool);
    fn set_accel_profile(&self, profile: InputDeviceAccelProfile);
    fn set_accel_speed(&self, speed: f64);
    fn set_transform_matrix(&self, matrix: [[f64; 2]; 2]);
    fn name(&self) -> Rc<String>;
}

#[derive(Debug, Copy, Clone)]
pub enum InputDeviceCapability {
    Keyboard,
    Pointer,
    Touch,
    TabletTool,
    TabletPad,
    Gesture,
    Switch,
}

#[derive(Debug, Copy, Clone)]
pub enum InputDeviceAccelProfile {
    Flat,
    Adaptive,
}

pub enum BackendEvent {
    NewConnector(Rc<dyn Connector>),
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
