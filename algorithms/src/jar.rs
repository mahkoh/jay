use crate::mmap::Mmapped;
use crate::mmap::mmap;
use crate::oserror::OsError;
use crate::oserror::OsErrorExt2;
use rustc_hash::FxBuildHasher;
use std::collections::HashSet;
use std::io;
use std::io::BufWriter;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::ptr;
use std::slice;
use thiserror::Error;
use uapi::OwnedFd;
use uapi::c;

#[cfg(test)]
mod tests;

trait WriteAndSeek: Write + Seek {}

impl<T> WriteAndSeek for T where T: Write + Seek {}

pub struct JarWriter<'a> {
    w: &'a mut BufWriter<dyn WriteAndSeek>,
    linked: HashSet<u64, FxBuildHasher>,
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
    non_unique: HashSet<u64, FxBuildHasher>,
}

pub enum JarEvent<'a> {
    Dir(&'a [u8]),
    DirUp,
    Reg(&'a [u8], Option<u64>, &'a [u8]),
    Lnk(&'a [u8], &'a [u8]),
    Hrd(&'a [u8], u64),
}

const D: u8 = b'D';
const U: u8 = b'U';
const H: u8 = b'H';
const L: u8 = b'L';
const R: u8 = b'R';

impl<'a> JarWriter<'a> {
    pub fn new(w: &'a mut BufWriter<impl Write + Seek + 'static>) -> io::Result<Self> {
        let mut slf = Self {
            w,
            linked: HashSet::default(),
        };
        slf.write_u64(0)?;
        Ok(slf)
    }

    pub fn finish(&mut self) -> io::Result<()> {
        self.w.flush()?;
        let pos = self.w.get_mut().stream_position()?;
        for &n in self.linked.iter() {
            self.w.write_all(&n.to_le_bytes())?;
        }
        self.w.flush()?;
        self.w.get_mut().seek(SeekFrom::Start(0))?;
        self.write_u64(pos)?;
        Ok(())
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

    pub fn add_reg(&mut self, path: &[u8], unique: u64, contents: &[u8]) -> io::Result<()> {
        self.w.write_all(&[R])?;
        self.write_u64(path.len() as u64)?;
        self.w.write_all(path)?;
        self.write_u64(unique)?;
        self.write_u64(contents.len() as u64)?;
        self.w.write_all(contents)?;
        Ok(())
    }

    pub fn add_hrd(&mut self, path: &[u8], unique: u64) -> io::Result<()> {
        self.linked.insert(unique);
        self.w.write_all(&[H])?;
        self.write_u64(path.len() as u64)?;
        self.w.write_all(path)?;
        self.write_u64(unique)?;
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

const SIZE_OF_U64: usize = size_of::<u64>();

impl JarReader {
    pub fn new(fd: &OwnedFd) -> Result<Self, JarError> {
        let stat = uapi::fstat(fd.raw()).map_os_err(JarError::Stat)?;
        let mmapped = mmap(stat.st_size as _, c::PROT_READ, c::MAP_PRIVATE, fd.raw(), 0)
            .map_err(JarError::Mmap)?;
        let ptr = mmapped.ptr.cast();
        let len = mmapped.ptr.len();
        let mut slf = Self {
            ptr,
            len,
            _mmapped: Some(mmapped),
            non_unique: Default::default(),
        };
        let non_unique_offset = slf.read_u64()?;
        if non_unique_offset > len as u64 {
            return Err(JarError::Corrupt);
        }
        let non_unique_offset = non_unique_offset as usize;
        if non_unique_offset < SIZE_OF_U64 {
            return Err(JarError::Corrupt);
        }
        let non_unique_len = len - non_unique_offset;
        let slice = unsafe { slice::from_raw_parts(ptr.add(non_unique_offset), non_unique_len) };
        let Ok(iter) = uapi::pod_iter::<u64, _>(slice) else {
            return Err(JarError::Corrupt);
        };
        slf.non_unique.extend(iter);
        slf.len -= non_unique_len;
        Ok(slf)
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
                let unique = self.read_u64()?;
                let contents = self.read_bytes()?;
                let unique = self.non_unique.contains(&unique).then_some(unique);
                unsafe { JarEvent::Reg(&*path, unique, &*contents) }
            }
            H => {
                let path = self.read_bytes()?;
                let unique = self.read_u64()?;
                unsafe { JarEvent::Hrd(&*path, unique) }
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
        if self.len < SIZE_OF_U64 {
            return Err(JarError::Corrupt);
        }
        let len = unsafe { self.ptr.cast::<[u8; SIZE_OF_U64]>().read() };
        let len = u64::from_le_bytes(len);
        self.consume(SIZE_OF_U64);
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
