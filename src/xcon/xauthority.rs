use {
    crate::xcon::XconError,
    bstr::{BString, ByteSlice},
    std::{fs::File, io::Read},
};

pub const LOCAL: u16 = 256;

pub const MIT_MAGIC_COOKIE: &[u8] = b"MIT-MAGIC-COOKIE-1";

#[derive(Debug)]
pub struct XAuthority {
    pub family: u16,
    pub host: BString,
    pub display: u32,
    pub method: BString,
    pub value: BString,
}

impl XAuthority {
    pub fn load() -> Result<Vec<XAuthority>, XconError> {
        let path = 'path: {
            if let Ok(p) = std::env::var("XAUTHORITY") {
                break 'path p;
            }
            if let Ok(home) = std::env::var("HOME") {
                break 'path format!("{home}/.Xauthority");
            }
            return Err(XconError::HomeNotSet);
        };
        let mut buf = vec![];
        if let Err(e) = File::open(path).and_then(|mut f| f.read_to_end(&mut buf)) {
            return Err(XconError::ReadXAuthority(e));
        }
        Parser::parse(&buf)
    }
}

struct Parser<'a> {
    pos: usize,
    buf: &'a [u8],
}

impl<'a> Parser<'a> {
    fn parse(buf: &[u8]) -> Result<Vec<XAuthority>, XconError> {
        let mut slf = Parser { pos: 0, buf };
        let mut res = vec![];
        while slf.rem() > 0 {
            res.push(slf.parse_one()?);
        }
        Ok(res)
    }

    fn rem(&self) -> usize {
        self.buf.len() - self.pos
    }

    fn parse_one(&mut self) -> Result<XAuthority, XconError> {
        Ok(XAuthority {
            family: self.read_u16()?,
            host: self.read_string()?,
            display: {
                let s = self.read_string()?;
                match s.to_str() {
                    Ok(s) => match s.parse() {
                        Ok(v) => v,
                        _ => return Err(XconError::InvalidAuthorityDisplay),
                    },
                    _ => return Err(XconError::InvalidAuthorityDisplay),
                }
            },
            method: self.read_string()?,
            value: self.read_string()?,
        })
    }

    fn read_u16(&mut self) -> Result<u16, XconError> {
        if self.rem() < 2 {
            return Err(XconError::UnexpectedEof);
        }
        let bytes = [self.buf[self.pos], self.buf[self.pos + 1]];
        self.pos += 2;
        Ok(u16::from_be_bytes(bytes))
    }

    fn read_string(&mut self) -> Result<BString, XconError> {
        let len = self.read_u16()? as usize;
        if self.rem() < len {
            log::info!("rem = {}; len = {}", self.rem(), len);
            return Err(XconError::UnexpectedEof);
        }
        let res = self.buf[self.pos..self.pos + len].to_vec();
        self.pos += len;
        let res = res.into();
        Ok(res)
    }
}
