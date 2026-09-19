use crate::tree::TreeTimeline;
use crate::tree::TreeTimeline::LiveTL;
use crate::tree::TreeTimeline::RenderTL;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;

#[expect(unused)]
pub fn format_path_link(buf: &mut String, depth: u64, category: &str, name: &str) {
    write_root_link(buf, depth);
    buf.push_str(category);
    buf.push_str("/");
    buf.push_str(name);
}

#[expect(unused)]
pub fn write_root_link(buf: &mut String, depth: u64) {
    for _ in 1..depth {
        buf.push_str("../");
    }
}

#[expect(unused)]
pub fn dfs_split_view<T>(dst: &mut String, ctx: &StrCtx, mut f: impl FnMut(TreeTimeline) -> T)
where
    T: StrFmt,
{
    ctx.struct_prefix(dst);
    ctx.struct_field(dst, "live", &f(LiveTL), true);
    ctx.struct_field(dst, "rndr", &f(RenderTL), false);
    ctx.struct_suffix(dst);
}
