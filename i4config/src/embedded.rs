use crate::InputDevice;

pub fn grab_input_device(kb: InputDevice, grab: bool) {
    get!().grab(kb, grab);
}
