use std::io;
use std::io::BufWriter;
use std::io::Write;

pub struct JarWriter<'a> {
    w: &'a mut BufWriter<dyn Write>,
}

impl<'a> JarWriter<'a> {
    pub fn new(w: &'a mut BufWriter<dyn Write>) -> Self {
        Self { w }
    }

    pub fn add_dir(&mut self, path: &[u8]) -> io::Result<()> {
        self.w.write_all(b"D")?;
        self.w.write_all(&(path.len() as u64).to_le_bytes())?;
        self.w.write_all(path)?;
        Ok(())
    }

    pub fn add_dir_up(&mut self) -> io::Result<()> {
        self.w.write_all(b"U")?;
        Ok(())
    }

    pub fn add_reg(&mut self, path: &[u8], contents: &[u8]) -> io::Result<()> {
        self.w.write_all(b"R")?;
        self.w.write_all(&(path.len() as u64).to_le_bytes())?;
        self.w.write_all(path)?;
        self.w.write_all(&(contents.len() as u64).to_le_bytes())?;
        self.w.write_all(contents)?;
        Ok(())
    }

    pub fn add_lnk(&mut self, path: &[u8], linkpath: &[u8]) -> io::Result<()> {
        self.w.write_all(b"L")?;
        self.w.write_all(&(path.len() as u64).to_le_bytes())?;
        self.w.write_all(path)?;
        self.w.write_all(&(linkpath.len() as u64).to_le_bytes())?;
        self.w.write_all(linkpath)?;
        Ok(())
    }
}
