use std::io;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use uapi::c;

#[expect(unused)]
pub struct SeekableFd(pub c::c_int);

impl Write for SeekableFd {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let len = uapi::write(self.0, buf)?;
        Ok(len)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Seek for SeekableFd {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let (pos, whence) = match pos {
            SeekFrom::Start(n) => (n as _, c::SEEK_SET),
            SeekFrom::End(n) => (n as _, c::SEEK_END),
            SeekFrom::Current(n) => (n as _, c::SEEK_CUR),
        };
        let pos = uapi::lseek(self.0, pos, whence)?;
        Ok(pos as u64)
    }
}
