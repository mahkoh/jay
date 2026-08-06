use std::io;
use std::io::Read;

pub trait ReadExt {
    fn read_to_vec(&mut self) -> io::Result<Vec<u8>>;
}

impl<T> ReadExt for T
where
    T: Read,
{
    fn read_to_vec(&mut self) -> io::Result<Vec<u8>> {
        let mut res = vec![];
        self.read_to_end(&mut res)?;
        Ok(res)
    }
}
