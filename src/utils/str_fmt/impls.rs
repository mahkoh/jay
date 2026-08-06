use crate::fixed::Fixed;
use crate::utils::spaces::spaces;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::str_fmt::StrFmtFmt;
use bstr::BStr;
use std::borrow::Borrow;
use std::borrow::Cow;

impl<T> StrFmt for [T]
where
    T: StrFmt,
{
    fn str_fmt(&self, dst: &mut String, ctx: &StrCtx) {
        match ctx.fmt {
            StrFmtFmt::Human => {
                let cctx = &StrCtx {
                    spaces: &spaces(ctx.spaces.len() + 2),
                    ..*ctx
                };
                for (idx, v) in self.iter().enumerate() {
                    if idx > 0 {
                        dst.push_str("\n");
                        dst.push_str(&ctx.prefix);
                        dst.push_str(&ctx.spaces);
                    }
                    dst.push_str("- ");
                    v.str_fmt(dst, cctx);
                }
            }
            StrFmtFmt::Jsonl => {
                dst.push_str("[");
                for (idx, v) in self.iter().enumerate() {
                    if idx > 0 {
                        dst.push_str(",");
                    }
                    v.str_fmt(dst, ctx)
                }
                dst.push_str("]");
            }
            StrFmtFmt::Trace => {
                dst.push_str("[");
                for (idx, v) in self.iter().enumerate() {
                    if idx > 0 {
                        dst.push_str(", ");
                    }
                    v.str_fmt(dst, ctx);
                }
                dst.push_str("]");
            }
        }
    }
}

impl StrFmt for BStr {
    fn str_fmt(&self, dst: &mut String, ctx: &StrCtx) {
        dst.push_str("\"");
        for chunk in self.utf8_chunks() {
            fmt_str(dst, chunk.valid(), ctx);
            for _ in 0..chunk.invalid().len() {
                dst.push_str("�");
            }
        }
        dst.push_str("\"");
    }
}

impl StrFmt for str {
    fn str_fmt(&self, dst: &mut String, ctx: &StrCtx) {
        dst.push_str("\"");
        fmt_str(dst, self, ctx);
        dst.push_str("\"");
    }
}

pub fn fmt_str(dst: &mut String, b: &str, ctx: &StrCtx) {
    let dst = unsafe { dst.as_mut_vec() };
    for &b in b.as_bytes() {
        match b {
            b'\n' => dst.extend_from_slice(b"\\n"),
            b'\r' => dst.extend_from_slice(b"\\r"),
            b'\t' => dst.extend_from_slice(b"\\t"),
            b'\x00'..=b'\x1f' => {
                if ctx.fmt == StrFmtFmt::Jsonl {
                    dst.extend_from_slice(b"\\u00");
                } else {
                    dst.extend_from_slice(b"\\x");
                }
                static HEX: [u8; 16] = *b"0123456789abcdef";
                let lo = ((b >> 0) & 0xf) as usize;
                let hi = ((b >> 4) & 0xf) as usize;
                dst.push(HEX[hi]);
                dst.push(HEX[lo]);
            }
            b'"' => dst.extend_from_slice(b"\\\""),
            b'\\' => dst.extend_from_slice(b"\\\\"),
            _ => dst.push(b),
        }
    }
}

macro_rules! integer {
    ($ty:ty) => {
        impl StrFmt for $ty {
            fn str_fmt(&self, dst: &mut String, _ctx: &StrCtx) {
                let mut buf = itoa::Buffer::new();
                let str = buf.format(*self);
                dst.push_str(str);
            }
        }
    };
}

integer!(u8);
integer!(u16);
integer!(u32);
integer!(u64);
integer!(u128);
integer!(usize);
integer!(i8);
integer!(i16);
integer!(i32);
integer!(i64);
integer!(i128);
integer!(isize);

macro_rules! float {
    ($ty:ty) => {
        impl StrFmt for $ty {
            fn str_fmt(&self, dst: &mut String, _ctx: &StrCtx) {
                let mut buf = zmij::Buffer::new();
                let str = buf.format(*self);
                dst.push_str(str);
            }
        }
    };
}

float!(f32);
float!(f64);

impl StrFmt for Fixed {
    fn str_fmt(&self, dst: &mut String, ctx: &StrCtx) {
        self.to_f64().str_fmt(dst, ctx);
    }
}

impl StrFmt for bool {
    fn str_fmt(&self, dst: &mut String, _ctx: &StrCtx) {
        match *self {
            true => dst.push_str("true"),
            false => dst.push_str("false"),
        }
    }
}

impl<T> StrFmt for &T
where
    T: StrFmt + ?Sized,
{
    fn str_fmt(&self, dst: &mut String, ctx: &StrCtx) {
        T::str_fmt(*self, dst, ctx)
    }
}

impl<T> StrFmt for Option<T>
where
    T: StrFmt,
{
    fn str_fmt(&self, dst: &mut String, ctx: &StrCtx) {
        match self {
            None if ctx.fmt == StrFmtFmt::Jsonl => dst.push_str("null"),
            None => dst.push_str("nil"),
            Some(v) => v.str_fmt(dst, ctx),
        }
    }
}

impl<T> StrFmt for Box<T>
where
    T: StrFmt + ?Sized,
{
    fn str_fmt(&self, dst: &mut String, ctx: &StrCtx) {
        T::str_fmt(self, dst, ctx)
    }
}

impl<T> StrFmt for Cow<'_, T>
where
    T: ToOwned + StrFmt + ?Sized,
{
    fn str_fmt(&self, dst: &mut String, ctx: &StrCtx) {
        let t: &T = self.borrow();
        t.str_fmt(dst, ctx)
    }
}
