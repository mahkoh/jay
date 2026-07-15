use crate::update;
use anyhow::Result;
use std::fmt;
use std::fmt::Write;

pub fn main() -> Result<()> {
    let simple = [
        ("Linear", "linear"),
        ("St2084Pq", "st2084_pq"),
        ("Gamma22", "gamma22"),
        ("Gamma24", "gamma24"),
        ("Gamma28", "gamma28"),
        ("St240", "st240"),
        ("Log100", "log100"),
        ("Log316", "log316"),
        ("St428", "st428"),
        ("CompoundPower24", "compound_power_2_4"),
    ];
    let complex = [("Bt1886", "bt1886"), ("Pow", "pow")];
    let mut out = String::new();
    define_w!(out);
    wl!("use crate::lut::fill;");
    wl!("use crate::tf::*;");
    wl!();
    fn fill(args: fmt::Arguments) -> impl fmt::Display {
        fmt::from_fn(move |fmt| write!(fmt, "fill(res, src_mul, dst_mul, {args})"))
    }
    let mut generate_single = |name: &str, module: &str| {
        wl!("#[inline(never)]");
        wl!("pub fn {name}(res: &mut [f32], src_mul: f32, dst_mul: f32, i: AlgoEotf) {{");
        wl!("    match i {{");
        for (te, tf) in simple {
            wl!(
                "            AlgoEotf::{te} => {},",
                fill(format_args!("{module}::{tf}::<()>")),
            );
        }
        for (te, tf) in complex {
            wl!("            AlgoEotf::{te}(tp) => {{");
            wl!("                let tf = {module}::{tf}::<()>(tp);");
            wl!("                {}", fill(format_args!("tf")));
            wl!("            }}");
        }
        wl!("    }}");
        wl!("}}");
        wl!();
        Ok::<(), fmt::Error>(())
    };
    generate_single("fill_eotf", "eotfs")?;
    generate_single("fill_inv_eotf", "inv_eotfs")?;
    wl!("#[inline(never)]");
    wl!(
        "pub fn fill_dual(res: &mut [f32], src_mul: f32, dst_mul: f32, i: AlgoEotf, o: AlgoEotf) {{"
    );
    wl!("#[inline(always)]");
    wl!(
        "fn handle_dst(res: &mut [f32], src_mul: f32, dst_mul: f32, o: AlgoEotf, sf: impl Fn(f32) -> f32 + Copy) {{",
    );
    wl!("    match o {{");
    for (te, tf) in simple {
        wl!("AlgoEotf::{te} => {{");
        wl!(
            "    {};",
            fill(format_args!("|v| inv_eotfs::{tf}::<()>(sf(v))"))
        );
        wl!("}}");
    }
    for (te, tf) in complex {
        wl!("AlgoEotf::{te}(tp) => {{");
        wl!("    #[inline(never)]");
        wl!(
            "    fn f(res: &mut [f32], src_mul: f32, dst_mul: f32, tp: f32, sf: impl Fn(f32) -> f32 + Copy) {{"
        );
        wl!("        let tf = inv_eotfs::{tf}::<()>(tp);");
        wl!("        {}", fill(format_args!("|v| tf(sf(v))")));
        wl!("    }}");
        wl!("    f(res, src_mul, dst_mul, tp, sf);");
        wl!("}}");
    }
    wl!("    }}");
    wl!("}}");
    wl!("    match i {{");
    for (se, sf) in simple {
        wl!("AlgoEotf::{se} => {{");
        wl!("    handle_dst(res, src_mul, dst_mul, o, eotfs::{sf}::<()>);");
        wl!("}}");
    }
    for (se, sf) in complex {
        wl!("AlgoEotf::{se}(sp) => {{");
        wl!("    #[inline(never)]");
        wl!("    fn f(res: &mut [f32], src_mul: f32, dst_mul: f32, o: AlgoEotf, sp: f32) {{");
        wl!("        let sf = eotfs::{sf}::<()>(sp);");
        wl!("        handle_dst(res, src_mul, dst_mul, o, sf);");
        wl!("    }}");
        wl!("    f(res, src_mul, dst_mul, o, sp);");
        wl!("}}");
    }
    wl!("    }}");
    wl!("}}");
    update("algorithms/src/lut/generated.rs", &out)?;
    Ok(())
}
