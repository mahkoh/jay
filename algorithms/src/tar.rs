use bstr::ByteSlice;
use static_assertions::const_assert_eq;
use std::io;
use std::io::BufWriter;
use std::io::Write;
use uapi::Packed;

#[cfg(test)]
pub mod tests;

pub struct TarWriter<'a> {
    w: &'a mut BufWriter<dyn Write>,
    finished: Finished,
}

struct Finished(bool);

static HEADER_INIT: header_posix_ustar = header_posix_ustar {
    name: [0; 100],
    mode: [0; 8],
    uid: *b"0000000\0",
    gid: *b"0000000\0",
    size: [0; 12],
    mtime: *b"00000000000\0",
    checksum: *b"        ",
    typeflag: [0; 1],
    linkname: [0; 100],
    magic: *b"ustar\0",
    version: *b"00",
    uname: [0; 32],
    gname: [0; 32],
    devmajor: [0; 8],
    devminor: [0; 8],
    prefix: [0; 155],
    pad: [0; 12],
};

const RECORD_SIZE: usize = 512;

const MAX_SIZE: u64 = 1 << 33;

static ZERO_RECORD: [u8; RECORD_SIZE] = [0; RECORD_SIZE];

impl<'a> TarWriter<'a> {
    pub fn new(w: &'a mut BufWriter<dyn Write>) -> Self {
        Self {
            w,
            finished: Finished(false),
        }
    }

    pub fn add_dir(&mut self, path: &[u8]) -> io::Result<()> {
        let path = split_path(path);
        self.write_ext(path.err(), None, None)?;
        let mut header = HEADER_INIT;
        if let Ok((prefix, name)) = path {
            header.write_path(prefix, name);
        }
        let _ = write!(&mut header.mode[..], "{:07o}", 0o755);
        header.typeflag = *b"5";
        header.write_checksum();
        self.w.write_all(uapi::as_bytes(&header))?;
        Ok(())
    }

    pub fn add_reg(&mut self, path: &[u8], contents: &[u8]) -> io::Result<()> {
        self.add_reg_max(path, contents, MAX_SIZE)
    }

    fn add_reg_max(&mut self, path: &[u8], contents: &[u8], max: u64) -> io::Result<()> {
        let path = split_path(path);
        let size = contents.len() as u64;
        let size = if size >= max { Err(size) } else { Ok(size) };
        self.write_ext(path.err(), None, size.err())?;
        let mut header = HEADER_INIT;
        if let Ok((prefix, name)) = path {
            header.write_path(prefix, name);
        }
        let _ = write!(&mut header.mode[..], "{:07o}", 0o644);
        if let Ok(size) = size {
            let _ = write!(&mut header.size[..], "{:011o}", size);
        }
        header.typeflag = *b"0";
        header.write_checksum();
        self.w.write_all(uapi::as_bytes(&header))?;
        self.w.write_all(contents)?;
        let rem = contents.len().wrapping_neg() & (RECORD_SIZE - 1);
        self.w.write_all(&ZERO_RECORD[..rem])?;
        Ok(())
    }

    pub fn add_lnk(&mut self, path: &[u8], linkpath: &[u8]) -> io::Result<()> {
        let path = split_path(path);
        let linkpath = if linkpath.len() > 100 {
            Err(linkpath)
        } else {
            Ok(linkpath)
        };
        self.write_ext(path.err(), linkpath.err(), None)?;
        let mut header = HEADER_INIT;
        if let Ok((prefix, name)) = path {
            header.write_path(prefix, name);
        }
        if let Ok(linkpath) = linkpath {
            header.linkname[..linkpath.len()].copy_from_slice(linkpath);
        }
        let _ = write!(&mut header.mode[..], "{:07o}", 0o777);
        header.typeflag = *b"2";
        header.write_checksum();
        self.w.write_all(uapi::as_bytes(&header))?;
        Ok(())
    }

    pub fn finish(mut self) -> io::Result<()> {
        self.finished.0 = true;
        self.w.write_all(&ZERO_RECORD)?;
        self.w.write_all(&ZERO_RECORD)?;
        Ok(())
    }

    fn write_ext(
        &mut self,
        path: Option<&[u8]>,
        linkpath: Option<&[u8]>,
        size: Option<u64>,
    ) -> io::Result<()> {
        const DECIMAL_SIZE: usize = 20;
        const PATH: &str = "path";
        const LINKPATH: &str = "linkpath";
        const SIZE: &str = "size";
        let mut path_size = 0;
        let mut linkpath_size = 0;
        let mut size_size = 0;
        if let Some(path) = path {
            path_size = DECIMAL_SIZE + 1 + PATH.len() + 1 + path.len() + 1;
        }
        if let Some(linkpath) = linkpath {
            linkpath_size = DECIMAL_SIZE + 1 + LINKPATH.len() + 1 + linkpath.len() + 1;
        }
        if size.is_some() {
            size_size = DECIMAL_SIZE + 1 + SIZE.len() + 1 + DECIMAL_SIZE + 1;
        }
        let ext_size = path_size + linkpath_size + size_size;
        if ext_size == 0 {
            return Ok(());
        }
        let mut header = HEADER_INIT;
        let _ = write!(&mut header.size[..], "{:011o}", ext_size);
        header.typeflag = *b"x";
        header.write_checksum();
        self.w.write_all(uapi::as_bytes(&header))?;
        let mut prefix = [0u8; DECIMAL_SIZE];
        let mut write_line = |name: &str, size: usize, val: &[u8]| {
            let _ = write!(&mut prefix[..], "{size:0width$}", width = DECIMAL_SIZE);
            self.w.write_all(&prefix)?;
            self.w.write_all(b" ")?;
            self.w.write_all(name.as_bytes())?;
            self.w.write_all(b"=")?;
            self.w.write_all(val)?;
            self.w.write_all(b"\n")?;
            io::Result::Ok(())
        };
        if let Some(path) = path {
            write_line(PATH, path_size, path)?;
        }
        if let Some(linkpath) = linkpath {
            write_line(LINKPATH, linkpath_size, linkpath)?;
        }
        if let Some(size) = size {
            let mut tmp = [0; DECIMAL_SIZE];
            let _ = write!(&mut tmp[..], "{size:0width$}", width = DECIMAL_SIZE);
            write_line(SIZE, size_size, &tmp)?;
        }
        let rem = ext_size.wrapping_neg() & (RECORD_SIZE - 1);
        self.w.write_all(&ZERO_RECORD[..rem])?;
        Ok(())
    }
}

impl Drop for Finished {
    fn drop(&mut self) {
        if !self.0 {
            log::warn!("Tar archive was not finished");
        }
    }
}

fn split_path(path: &[u8]) -> Result<(&[u8], &[u8]), &[u8]> {
    if path.len() <= 100 {
        return Ok((&[], path));
    }
    if path.len() > 256 {
        return Err(path);
    }
    for v in path.rfind_iter(b"/") {
        if v == 0 {
            return Err(path);
        }
        if path.len() - v - 1 > 100 {
            return Err(path);
        }
        if v > 155 {
            continue;
        }
        return Ok((&path[..v], &path[(v + 1)..]));
    }
    Err(path)
}

#[repr(C)]
#[derive(Copy, Clone)]
struct header_posix_ustar {
    name: [u8; 100],
    mode: [u8; 8],
    uid: [u8; 8],
    gid: [u8; 8],
    size: [u8; 12],
    mtime: [u8; 12],
    checksum: [u8; 8],
    typeflag: [u8; 1],
    linkname: [u8; 100],
    magic: [u8; 6],
    version: [u8; 2],
    uname: [u8; 32],
    gname: [u8; 32],
    devmajor: [u8; 8],
    devminor: [u8; 8],
    prefix: [u8; 155],
    pad: [u8; 12],
}

const_assert_eq!(size_of::<header_posix_ustar>(), RECORD_SIZE);

unsafe impl Packed for header_posix_ustar {}

impl header_posix_ustar {
    fn write_path(&mut self, prefix: &[u8], name: &[u8]) {
        self.name[..name.len()].copy_from_slice(name);
        self.prefix[..prefix.len()].copy_from_slice(prefix);
    }

    fn checksum(&self) -> u32 {
        let mut checksum = 0u32;
        for b in uapi::as_bytes(self) {
            checksum = checksum.wrapping_add(*b as u32);
        }
        checksum
    }

    fn write_checksum(&mut self) {
        let checksum = self.checksum();
        let _ = write!(&mut self.checksum[..], "{:06o}\0 ", checksum);
    }
}
