use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::str_fmt::StrFmtFmt;
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

impl StrFmt for MajorMinor {
    fn str_fmt(&self, dst: &mut String, ctx: &StrCtx) {
        let mut buf = itoa::Buffer::new();
        if ctx.fmt == StrFmtFmt::Jsonl {
            dst.push_str("\"");
        }
        dst.push_str(buf.format(self.major));
        dst.push_str(":");
        dst.push_str(buf.format(self.minor));
        if ctx.fmt == StrFmtFmt::Jsonl {
            dst.push_str("\"");
        }
    }
}
