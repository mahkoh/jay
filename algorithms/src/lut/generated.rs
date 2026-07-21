use crate::lut::fill;
use crate::tf::*;

#[inline(never)]
pub fn fill_eotf(res: &mut [f32], src_mul: f32, dst_mul: f32, i: AlgoEotf) {
    match i {
        AlgoEotf::Linear => fill(res, src_mul, dst_mul, eotfs::linear::<()>),
        AlgoEotf::St2084Pq => fill(res, src_mul, dst_mul, eotfs::st2084_pq::<()>),
        AlgoEotf::Gamma22 => fill(res, src_mul, dst_mul, eotfs::gamma22::<()>),
        AlgoEotf::Gamma24 => fill(res, src_mul, dst_mul, eotfs::gamma24::<()>),
        AlgoEotf::Gamma28 => fill(res, src_mul, dst_mul, eotfs::gamma28::<()>),
        AlgoEotf::St240 => fill(res, src_mul, dst_mul, eotfs::st240::<()>),
        AlgoEotf::Log100 => fill(res, src_mul, dst_mul, eotfs::log100::<()>),
        AlgoEotf::Log316 => fill(res, src_mul, dst_mul, eotfs::log316::<()>),
        AlgoEotf::St428 => fill(res, src_mul, dst_mul, eotfs::st428::<()>),
        AlgoEotf::CompoundPower24 => fill(res, src_mul, dst_mul, eotfs::compound_power_2_4::<()>),
        AlgoEotf::Bt1886(tp) => {
            let tf = eotfs::bt1886::<()>(tp);
            fill(res, src_mul, dst_mul, tf)
        }
        AlgoEotf::Pow(tp) => {
            let tf = eotfs::pow::<()>(tp);
            fill(res, src_mul, dst_mul, tf)
        }
    }
}

#[inline(never)]
pub fn fill_inv_eotf(res: &mut [f32], src_mul: f32, dst_mul: f32, i: AlgoEotf) {
    match i {
        AlgoEotf::Linear => fill(res, src_mul, dst_mul, inv_eotfs::linear::<()>),
        AlgoEotf::St2084Pq => fill(res, src_mul, dst_mul, inv_eotfs::st2084_pq::<()>),
        AlgoEotf::Gamma22 => fill(res, src_mul, dst_mul, inv_eotfs::gamma22::<()>),
        AlgoEotf::Gamma24 => fill(res, src_mul, dst_mul, inv_eotfs::gamma24::<()>),
        AlgoEotf::Gamma28 => fill(res, src_mul, dst_mul, inv_eotfs::gamma28::<()>),
        AlgoEotf::St240 => fill(res, src_mul, dst_mul, inv_eotfs::st240::<()>),
        AlgoEotf::Log100 => fill(res, src_mul, dst_mul, inv_eotfs::log100::<()>),
        AlgoEotf::Log316 => fill(res, src_mul, dst_mul, inv_eotfs::log316::<()>),
        AlgoEotf::St428 => fill(res, src_mul, dst_mul, inv_eotfs::st428::<()>),
        AlgoEotf::CompoundPower24 => {
            fill(res, src_mul, dst_mul, inv_eotfs::compound_power_2_4::<()>)
        }
        AlgoEotf::Bt1886(tp) => {
            let tf = inv_eotfs::bt1886::<()>(tp);
            fill(res, src_mul, dst_mul, tf)
        }
        AlgoEotf::Pow(tp) => {
            let tf = inv_eotfs::pow::<()>(tp);
            fill(res, src_mul, dst_mul, tf)
        }
    }
}

#[inline(never)]
pub fn fill_dual(res: &mut [f32], src_mul: f32, dst_mul: f32, i: AlgoEotf, o: AlgoEotf) {
    #[inline(always)]
    fn handle_dst(
        res: &mut [f32],
        src_mul: f32,
        dst_mul: f32,
        o: AlgoEotf,
        sf: impl Fn(f32) -> f32 + Copy,
    ) {
        match o {
            AlgoEotf::Linear => {
                fill(res, src_mul, dst_mul, |v| inv_eotfs::linear::<()>(sf(v)));
            }
            AlgoEotf::St2084Pq => {
                fill(res, src_mul, dst_mul, |v| inv_eotfs::st2084_pq::<()>(sf(v)));
            }
            AlgoEotf::Gamma22 => {
                fill(res, src_mul, dst_mul, |v| inv_eotfs::gamma22::<()>(sf(v)));
            }
            AlgoEotf::Gamma24 => {
                fill(res, src_mul, dst_mul, |v| inv_eotfs::gamma24::<()>(sf(v)));
            }
            AlgoEotf::Gamma28 => {
                fill(res, src_mul, dst_mul, |v| inv_eotfs::gamma28::<()>(sf(v)));
            }
            AlgoEotf::St240 => {
                fill(res, src_mul, dst_mul, |v| inv_eotfs::st240::<()>(sf(v)));
            }
            AlgoEotf::Log100 => {
                fill(res, src_mul, dst_mul, |v| inv_eotfs::log100::<()>(sf(v)));
            }
            AlgoEotf::Log316 => {
                fill(res, src_mul, dst_mul, |v| inv_eotfs::log316::<()>(sf(v)));
            }
            AlgoEotf::St428 => {
                fill(res, src_mul, dst_mul, |v| inv_eotfs::st428::<()>(sf(v)));
            }
            AlgoEotf::CompoundPower24 => {
                fill(res, src_mul, dst_mul, |v| {
                    inv_eotfs::compound_power_2_4::<()>(sf(v))
                });
            }
            AlgoEotf::Bt1886(tp) => {
                #[inline(never)]
                fn f(
                    res: &mut [f32],
                    src_mul: f32,
                    dst_mul: f32,
                    tp: f32,
                    sf: impl Fn(f32) -> f32 + Copy,
                ) {
                    let tf = inv_eotfs::bt1886::<()>(tp);
                    fill(res, src_mul, dst_mul, |v| tf(sf(v)))
                }
                f(res, src_mul, dst_mul, tp, sf);
            }
            AlgoEotf::Pow(tp) => {
                #[inline(never)]
                fn f(
                    res: &mut [f32],
                    src_mul: f32,
                    dst_mul: f32,
                    tp: f32,
                    sf: impl Fn(f32) -> f32 + Copy,
                ) {
                    let tf = inv_eotfs::pow::<()>(tp);
                    fill(res, src_mul, dst_mul, |v| tf(sf(v)))
                }
                f(res, src_mul, dst_mul, tp, sf);
            }
        }
    }
    match i {
        AlgoEotf::Linear => {
            handle_dst(res, src_mul, dst_mul, o, eotfs::linear::<()>);
        }
        AlgoEotf::St2084Pq => {
            handle_dst(res, src_mul, dst_mul, o, eotfs::st2084_pq::<()>);
        }
        AlgoEotf::Gamma22 => {
            handle_dst(res, src_mul, dst_mul, o, eotfs::gamma22::<()>);
        }
        AlgoEotf::Gamma24 => {
            handle_dst(res, src_mul, dst_mul, o, eotfs::gamma24::<()>);
        }
        AlgoEotf::Gamma28 => {
            handle_dst(res, src_mul, dst_mul, o, eotfs::gamma28::<()>);
        }
        AlgoEotf::St240 => {
            handle_dst(res, src_mul, dst_mul, o, eotfs::st240::<()>);
        }
        AlgoEotf::Log100 => {
            handle_dst(res, src_mul, dst_mul, o, eotfs::log100::<()>);
        }
        AlgoEotf::Log316 => {
            handle_dst(res, src_mul, dst_mul, o, eotfs::log316::<()>);
        }
        AlgoEotf::St428 => {
            handle_dst(res, src_mul, dst_mul, o, eotfs::st428::<()>);
        }
        AlgoEotf::CompoundPower24 => {
            handle_dst(res, src_mul, dst_mul, o, eotfs::compound_power_2_4::<()>);
        }
        AlgoEotf::Bt1886(sp) => {
            #[inline(never)]
            fn f(res: &mut [f32], src_mul: f32, dst_mul: f32, o: AlgoEotf, sp: f32) {
                let sf = eotfs::bt1886::<()>(sp);
                handle_dst(res, src_mul, dst_mul, o, sf);
            }
            f(res, src_mul, dst_mul, o, sp);
        }
        AlgoEotf::Pow(sp) => {
            #[inline(never)]
            fn f(res: &mut [f32], src_mul: f32, dst_mul: f32, o: AlgoEotf, sp: f32) {
                let sf = eotfs::pow::<()>(sp);
                handle_dst(res, src_mul, dst_mul, o, sf);
            }
            f(res, src_mul, dst_mul, o, sp);
        }
    }
}
