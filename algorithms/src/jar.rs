use crate::mmap::Mmapped;
use crate::mmap::mmap;
use crate::oserror::OsError;
use crate::oserror::OsErrorExt2;
use std::io;
use std::io::BufWriter;
use std::io::Write;
use std::ptr;
use thiserror::Error;
use uapi::OwnedFd;
use uapi::c;

#[cfg(test)]
mod tests;

pub struct JarWriter<'a> {
    w: &'a mut BufWriter<dyn Write>,
}

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

const D: u8 = b'D';
const U: u8 = b'U';
const L: u8 = b'L';
const R: u8 = b'R';

impl<'a> JarWriter<'a> {
    pub fn new(w: &'a mut BufWriter<dyn Write>) -> Self {
        Self { w }
    }

    pub fn add_dir(&mut self, path: &[u8]) -> io::Result<()> {
        self.w.write_all(&[D])?;
        self.write_u64(path.len() as u64)?;
        self.w.write_all(path)?;
        Ok(())
    }

    pub fn add_dir_up(&mut self) -> io::Result<()> {
        self.w.write_all(&[U])?;
        Ok(())
    }

    pub fn add_reg(&mut self, path: &[u8], contents: &[u8]) -> io::Result<()> {
        self.w.write_all(&[R])?;
        self.write_u64(path.len() as u64)?;
        self.w.write_all(path)?;
        self.write_u64(contents.len() as u64)?;
        self.w.write_all(contents)?;
        Ok(())
    }

    pub fn add_lnk(&mut self, path: &[u8], linkpath: &[u8]) -> io::Result<()> {
        self.w.write_all(&[L])?;
        self.write_u64(path.len() as u64)?;
        self.w.write_all(path)?;
        self.write_u64(linkpath.len() as u64)?;
        self.w.write_all(linkpath)?;
        Ok(())
    }

    fn write_u64(&mut self, n: u64) -> io::Result<()> {
        self.w.write_all(&n.to_le_bytes())?;
        Ok(())
    }
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
            D => {
                let path = self.read_bytes()?;
                unsafe { JarEvent::Dir(&*path) }
            }
            U => JarEvent::DirUp,
            R => {
                let path = self.read_bytes()?;
                let contents = self.read_bytes()?;
                unsafe { JarEvent::Reg(&*path, &*contents) }
            }
            L => {
                let path = self.read_bytes()?;
                let linkpath = self.read_bytes()?;
                unsafe { JarEvent::Lnk(&*path, &*linkpath) }
            }
            _ => return Err(JarError::Corrupt),
        };
        Ok(Some(ev))
    }

    fn read_u64(&mut self) -> Result<u64, JarError> {
        if self.len < 8 {
            return Err(JarError::Corrupt);
        }
        let len = unsafe { self.ptr.cast::<[u8; 8]>().read() };
        let len = u64::from_le_bytes(len);
        self.consume(8);
        Ok(len)
    }

    fn read_bytes(&mut self) -> Result<*const [u8], JarError> {
        let len = self.read_u64()?;
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
