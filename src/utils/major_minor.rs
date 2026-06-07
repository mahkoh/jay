use uapi::c;

#[derive(Copy, Clone, Debug)]
pub struct MajorMinor {
    pub major: u64,
    pub minor: u64,
}

pub fn major_minor(dev_t: c::dev_t) -> MajorMinor {
    MajorMinor {
        major: uapi::major(dev_t),
        minor: uapi::minor(dev_t),
    }
}
