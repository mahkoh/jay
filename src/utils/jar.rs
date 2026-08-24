use crate::utils::mmap::Mmapped;
use crate::utils::mmap::mmap;
use jay_algorithms::oserror::OsError;
use jay_algorithms::oserror::OsErrorExt2;
use std::ptr;
use thiserror::Error;
use uapi::OwnedFd;
use uapi::c;

#[cfg(test)]
mod tests;

#[derive(Debug, Error)]
pub enum JarError {
    #[error("Could not stat")]
    Stat(#[source] OsError),
    #[error("Could not mmap")]
    Mmap(#[source] OsError),
    #[error("The memory is corrupted")]
    Corrupt,
}

pub struct JarReader {
    _mmapped: Option<Mmapped>,
    ptr: *const u8,
    len: usize,
}

pub enum JarEvent<'a> {
    Dir(&'a [u8]),
    DirUp,
    Reg(&'a [u8], &'a [u8]),
    Lnk(&'a [u8], &'a [u8]),
}

impl JarReader {
    pub fn new(fd: &OwnedFd) -> Result<Self, JarError> {
        let stat = uapi::fstat(fd.raw()).map_os_err(JarError::Stat)?;
        let ptr;
        let len;
        let mmapped;
        if stat.st_size == 0 {
            ptr = ptr::dangling();
            len = 0;
            mmapped = None;
        } else {
            let m = mmap(stat.st_size as _, c::PROT_READ, c::MAP_PRIVATE, fd.raw(), 0)
                .map_err(JarError::Mmap)?;
            ptr = m.ptr.cast();
            len = m.ptr.len();
            mmapped = Some(m);
        };
        Ok(Self {
            ptr,
            len,
            _mmapped: mmapped,
        })
    }

    pub fn next(&mut self) -> Result<Option<JarEvent<'_>>, JarError> {
        if self.len == 0 {
            return Ok(None);
        }
        let ty = unsafe { *self.ptr };
        self.consume(1);
        let ev = match ty {
            b'D' => {
                let path = self.read_bytes()?;
                unsafe { JarEvent::Dir(&*path) }
            }
            b'U' => JarEvent::DirUp,
            b'R' => {
                let path = self.read_bytes()?;
                let contents = self.read_bytes()?;
                unsafe { JarEvent::Reg(&*path, &*contents) }
            }
            b'L' => {
                let path = self.read_bytes()?;
                let linkpath = self.read_bytes()?;
                unsafe { JarEvent::Lnk(&*path, &*linkpath) }
            }
            _ => return Err(JarError::Corrupt),
        };
        Ok(Some(ev))
    }

    fn read_bytes(&mut self) -> Result<*const [u8], JarError> {
        if self.len < 8 {
            return Err(JarError::Corrupt);
        }
        let len = unsafe { self.ptr.cast::<[u8; 8]>().read() };
        let len = u64::from_le_bytes(len);
        self.consume(8);
        if (self.len as u64) < len {
            return Err(JarError::Corrupt);
        }
        let len = len as usize;
        let string = ptr::slice_from_raw_parts(self.ptr, len);
        self.consume(len);
        Ok(string)
    }

    fn consume(&mut self, n: usize) {
        unsafe {
            self.len -= n;
            self.ptr = self.ptr.add(n);
        }
    }
}
