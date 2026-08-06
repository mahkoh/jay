use crate::utils::spaces::spaces;
use serde::Deserialize;
use serde::Serialize;
use std::fmt::Debug;
use std::fmt::Write;
use std::time::Duration;
use std::time::SystemTime;

mod impls;

pub trait StrFmt {
    fn str_fmt(&self, dst: &mut String, ctx: &StrCtx);
}

#[expect(unused)]
pub trait StrFmtExt {
    fn to_str_fmt(&self, ctx: &StrCtx) -> String;
}

impl<T> StrFmtExt for T
where
    T: StrFmt + ?Sized,
{
    fn to_str_fmt(&self, ctx: &StrCtx) -> String {
        let mut dst = String::new();
        self.str_fmt(&mut dst, ctx);
        dst
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub struct StrCtx<'a> {
    pub fmt: StrFmtFmt,
    pub prefix: &'a str,
    pub spaces: &'a str,
    pub flatten: bool,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub enum StrFmtFmt {
    #[default]
    Human,
    Jsonl,
    Trace,
}

impl StrCtx<'_> {
    pub fn struct_prefix(&self, dst: &mut String) {
        if self.fmt == StrFmtFmt::Jsonl && !self.flatten {
            dst.push_str("{");
        }
    }

    pub fn struct_suffix(&self, dst: &mut String) {
        if self.fmt == StrFmtFmt::Jsonl && !self.flatten {
            dst.push_str("}");
        }
    }

    pub fn struct_field(
        &self,
        dst: &mut String,
        name: &str,
        v: &(impl StrFmt + ?Sized),
        first: bool,
    ) {
        if self.fmt == StrFmtFmt::Jsonl {
            if !first {
                dst.push_str(",");
            }
            dst.push_str("\"");
            dst.push_str(name);
            dst.push_str("\":");
            let cctx = &StrCtx {
                flatten: false,
                ..*self
            };
            v.str_fmt(dst, cctx);
        } else {
            if !first {
                dst.push_str("\n");
                dst.push_str(self.prefix);
                dst.push_str(self.spaces);
            }
            dst.push_str(name);
            dst.push_str(": ");
            let cctx = &StrCtx {
                flatten: false,
                spaces: &spaces(self.spaces.len() + name.len() + 2),
                ..*self
            };
            v.str_fmt(dst, cctx);
        }
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct StrFmtUs(pub u64);

impl StrFmt for StrFmtUs {
    fn str_fmt(&self, dst: &mut String, ctx: &StrCtx) {
        if ctx.fmt == StrFmtFmt::Jsonl {
            self.0.str_fmt(dst, ctx);
        } else {
            let time = SystemTime::UNIX_EPOCH + Duration::from_micros(self.0);
            let _ = write!(dst, "{}", humantime::format_rfc3339_micros(time));
        }
    }
}
