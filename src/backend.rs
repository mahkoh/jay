use crate::fixed::Fixed;
use std::fmt::Debug;
use std::rc::Rc;

linear_ids!(OutputIds, OutputId);
linear_ids!(KeyboardIds, KeyboardId);
linear_ids!(MouseIds, MouseId);

pub trait Output {
    fn id(&self) -> OutputId;
    fn removed(&self) -> bool;
    fn width(&self) -> i32;
    fn height(&self) -> i32;
    fn on_change(&self, cb: Rc<dyn Fn()>);
}

pub trait Keyboard {
    fn id(&self) -> KeyboardId;
    fn removed(&self) -> bool;
    fn event(&self) -> Option<KeyboardEvent>;
    fn on_change(&self, cb: Rc<dyn Fn()>);
}

pub trait Mouse {
    fn id(&self) -> MouseId;
    fn removed(&self) -> bool;
    fn event(&self) -> Option<MouseEvent>;
    fn on_change(&self, cb: Rc<dyn Fn()>);
}

pub enum BackendEvent {
    NewOutput(Rc<dyn Output>),
    NewKeyboard(Rc<dyn Keyboard>),
    NewMouse(Rc<dyn Mouse>),
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
pub enum KeyboardEvent {
    Key(u32, KeyState),
}

#[derive(Debug)]
pub enum MouseEvent {
    OutputPosition(OutputId, Fixed, Fixed),
    #[allow(dead_code)]
    Motion(Fixed, Fixed),
    Button(u32, KeyState),
    Scroll(i32, ScrollAxis),
}
