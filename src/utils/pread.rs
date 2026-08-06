use std::io::Read;
use std::os::fd::AsRawFd;
use uapi::c;

pub struct Preader<T> {
    t: T,
    off: c::off_t,
}

impl<T> Preader<T> {
    pub fn new(t: T) -> Self {
        Self { t, off: 0 }
    }
}

impl<T> Read for Preader<T>
where
    T: AsRawFd,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = uapi::pread(self.t.as_raw_fd(), buf, self.off)?;
        let len = n.len();
        self.off += len as c::off_t;
        Ok(len)
    }
}
