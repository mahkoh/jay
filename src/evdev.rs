use crate::evdev::input_event_codes::InputEventCode;
use crate::evdev::input_event_codes::MAX_INPUT_EVENT_CODE;
use crate::utils::ioctl::ioctl;
use crate::utils::oserror::OsError;
use uapi::_IOC_READ;
use uapi::OwnedFd;

pub mod input_event_codes;

const EV_KEY: u32 = 0x01;

pub fn eviocgbit_key(fd: &OwnedFd) -> Result<Vec<InputEventCode>, OsError> {
    let mut buf = [0u8; (MAX_INPUT_EVENT_CODE + 1).div_ceil(u8::BITS as usize)];
    let nr = uapi::_IOC(
        _IOC_READ,
        b'E' as _,
        (0x20 + EV_KEY) as _,
        size_of_val(&buf) as _,
    );
    unsafe {
        ioctl(fd.raw(), nr, &mut buf)?;
    }
    let mut res = vec![];
    for (idx, b) in buf.into_iter().enumerate() {
        for bit in 0..u8::BITS {
            if b & (1 << bit) != 0
                && let Some(code) = InputEventCode::from_raw(idx as u32 * u8::BITS + bit)
            {
                res.push(code);
            }
        }
    }
    Ok(res)
}
