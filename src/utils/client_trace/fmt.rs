use crate::utils::client_trace::ClientTraceArgVal;
use crate::utils::client_trace::ClientTraceMsg;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::str_fmt::StrFmtFmt;
use crate::utils::str_table::OptionStrAccessExt;
use crate::utils::time_lt::SplitUsec;
use crate::utils::time_lt::split_usec;
use bstr::ByteSlice;

impl ClientTraceMsg<'_> {
    pub fn fmt_text(&self, dst: &mut String, ctx: &StrCtx, color: bool, client_id: u64) {
        let ctx = &StrCtx {
            fmt: StrFmtFmt::Trace,
            ..*ctx
        };
        const WL_DEBUG_COLOR_RESET: &str = "\x1b[0m";
        #[expect(unused)]
        const WL_DEBUG_COLOR_RED: &str = "\x1b[31m";
        const WL_DEBUG_COLOR_GREEN: &str = "\x1b[32m";
        const WL_DEBUG_COLOR_YELLOW: &str = "\x1b[33m";
        const WL_DEBUG_COLOR_BLUE: &str = "\x1b[34m";
        const WL_DEBUG_COLOR_MAGENTA: &str = "\x1b[35m";
        const WL_DEBUG_COLOR_CYAN: &str = "\x1b[36m";
        macro_rules! add_color {
            ($name:ident) => {
                if color {
                    dst.push_str($name);
                }
            };
        }
        let SplitUsec {
            hour,
            minute,
            second,
            usec_hi,
            usec_mi,
            usec_lo,
        } = split_usec(self.us);
        let arrow = match self.def.is_request {
            true => "->",
            false => "  ",
        };
        let interface = self.def.interface;
        let id = self.obj;
        let name = self.def.message;
        add_color!(WL_DEBUG_COLOR_GREEN);
        dst.push_str("[");
        dst.push_str(hour);
        dst.push_str(":");
        dst.push_str(minute);
        dst.push_str(":");
        dst.push_str(second);
        dst.push_str(".");
        dst.push_str(usec_hi);
        dst.push_str(usec_mi);
        dst.push_str(usec_lo);
        dst.push_str("] ");
        add_color!(WL_DEBUG_COLOR_YELLOW);
        dst.push_str("{");
        client_id.str_fmt(dst, ctx);
        dst.push_str("} ");
        add_color!(WL_DEBUG_COLOR_RESET);
        dst.push_str(arrow);
        dst.push_str(" ");
        add_color!(WL_DEBUG_COLOR_BLUE);
        dst.push_str(&interface);
        add_color!(WL_DEBUG_COLOR_MAGENTA);
        dst.push_str("#");
        id.str_fmt(dst, ctx);
        add_color!(WL_DEBUG_COLOR_CYAN);
        dst.push_str(".");
        dst.push_str(&name);
        add_color!(WL_DEBUG_COLOR_RESET);
        dst.push_str("(");
        for (idx, arg) in self.args.iter().enumerate() {
            if idx > 0 {
                dst.push_str(", ");
            }
            let name = arg.def.name;
            add_color!(WL_DEBUG_COLOR_YELLOW);
            dst.push_str(&name);
            dst.push_str(": ");
            add_color!(WL_DEBUG_COLOR_RESET);
            match arg.val {
                ClientTraceArgVal::Id(id) => {
                    let ty = arg.def.interface.get().unwrap_or("unknown");
                    add_color!(WL_DEBUG_COLOR_BLUE);
                    dst.push_str(ty);
                    add_color!(WL_DEBUG_COLOR_MAGENTA);
                    dst.push_str("#");
                    if id == 0 {
                        dst.push_str("nil");
                    } else {
                        id.str_fmt(dst, ctx);
                    }
                    add_color!(WL_DEBUG_COLOR_RESET);
                }
                ClientTraceArgVal::U32(n) => {
                    n.str_fmt(dst, ctx);
                }
                ClientTraceArgVal::I32(n) => {
                    n.str_fmt(dst, ctx);
                }
                ClientTraceArgVal::U64(n) => {
                    n.str_fmt(dst, ctx);
                }
                ClientTraceArgVal::Str(None) => {
                    dst.push_str("nil");
                }
                ClientTraceArgVal::Str(Some(n)) => {
                    n.as_bstr().str_fmt(dst, ctx);
                }
                ClientTraceArgVal::Fixed(n) => {
                    n.str_fmt(dst, ctx);
                }
                ClientTraceArgVal::Fd => {
                    dst.push_str("fd");
                }
                ClientTraceArgVal::Array(n) => {
                    n.str_fmt(dst, ctx);
                }
                ClientTraceArgVal::Pod(n) => {
                    n.str_fmt(dst, ctx);
                }
                ClientTraceArgVal::Bool(n) => {
                    n.str_fmt(dst, ctx);
                }
            }
        }
        dst.push_str(")\n");
    }

    pub fn fmt_jsonl(&self, dst: &mut String, ctx: &StrCtx, client_id: u64) {
        let ctx = &StrCtx {
            fmt: StrFmtFmt::Jsonl,
            ..*ctx
        };
        dst.push_str(r#"{"t":"m","cl":"#);
        client_id.str_fmt(dst, ctx);
        dst.push_str(r#","us":"#);
        self.us.str_fmt(dst, ctx);
        dst.push_str(r#","inf":""#);
        dst.push_str(&self.def.interface);
        dst.push_str(r#"","id":"#);
        self.obj.str_fmt(dst, ctx);
        dst.push_str(r#","msg":""#);
        dst.push_str(&self.def.message);
        dst.push_str(r#"","args":{"#);
        for (idx, arg) in self.args.iter().enumerate() {
            if idx > 0 {
                dst.push_str(r#","#);
            }
            dst.push_str(r#"""#);
            dst.push_str(&arg.def.name);
            dst.push_str(r#"":"#);
            match arg.val {
                ClientTraceArgVal::Id(n) => {
                    n.str_fmt(dst, ctx);
                }
                ClientTraceArgVal::U32(n) => {
                    n.str_fmt(dst, ctx);
                }
                ClientTraceArgVal::I32(n) => {
                    n.str_fmt(dst, ctx);
                }
                ClientTraceArgVal::U64(n) => {
                    n.str_fmt(dst, ctx);
                }
                ClientTraceArgVal::Str(None) => {
                    dst.push_str("null");
                }
                ClientTraceArgVal::Str(Some(n)) => {
                    n.as_bstr().str_fmt(dst, ctx);
                }
                ClientTraceArgVal::Fixed(n) => {
                    n.str_fmt(dst, ctx);
                }
                ClientTraceArgVal::Fd => {
                    dst.push_str("null");
                }
                ClientTraceArgVal::Array(n) => {
                    n.str_fmt(dst, ctx);
                }
                ClientTraceArgVal::Pod(n) => {
                    n.str_fmt(dst, ctx);
                }
                ClientTraceArgVal::Bool(n) => {
                    n.str_fmt(dst, ctx);
                }
            }
        }
        dst.push_str("}}\n");
    }
}
