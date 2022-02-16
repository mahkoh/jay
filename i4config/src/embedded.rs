use crate::Keyboard;

pub fn grab_keyboard(kb: Keyboard, grab: bool) {
    get!().grab(kb, grab);
}
