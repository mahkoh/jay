use crate::backend::BackendGammaLut;
use crate::cmm::cmm_eotf::Eotf;
use crate::utils::float_ext::FloatExt;
use jay_algorithms::lut::fill_dual;
use jay_algorithms::lut::fill_eotf;
use jay_algorithms::lut::fill_inv_eotf;
use jay_proc::jay_hash;
use std::rc::Rc;

pub trait MetalTfLutValue: Copy {
    const ZERO: Self;

    fn from_f32(v: f32) -> Self;
    fn from_f32_raw(v: f32) -> Self;
}

impl MetalTfLutValue for u16 {
    const ZERO: Self = 0;

    fn from_f32(v: f32) -> Self {
        (v * u16::MAX as f32) as u16
    }

    fn from_f32_raw(v: f32) -> Self {
        v as u16
    }
}

impl MetalTfLutValue for u32 {
    const ZERO: Self = 0;

    fn from_f32(v: f32) -> Self {
        (v as f64 * u32::MAX as f64) as u32
    }

    fn from_f32_raw(v: f32) -> Self {
        v as u32
    }
}

#[jay_hash]
#[derive(Copy, Clone, Debug)]
pub enum CurveConfig {
    Eotf(Eotf),
    InvEotf(Eotf),
    Both(Eotf, Eotf),
}

pub fn create_lut_curve(
    config: CurveConfig,
    size: usize,
    src_scale: Option<f32>,
    dst_scale: Option<f32>,
) -> Box<[f32]> {
    if size == 0 {
        return Default::default();
    }
    let mut res = vec![0.0; size].into_boxed_slice();
    let src_mul = src_scale.unwrap_or(1.0);
    let dst_mul = dst_scale.unwrap_or(1.0);
    match config {
        CurveConfig::Eotf(e) => fill_eotf(&mut res, src_mul, dst_mul, e.to_algo()),
        CurveConfig::InvEotf(e) => fill_inv_eotf(&mut res, src_mul, dst_mul, e.to_algo()),
        CurveConfig::Both(s, d) => fill_dual(&mut res, src_mul, dst_mul, s.to_algo(), d.to_algo()),
    }
    res
}

pub enum LutConfig<'a> {
    Curve(&'a [f32]),
    Size(usize),
}

pub fn create_lut<T>(
    config: LutConfig<'_>,
    post_lut: Option<&Rc<BackendGammaLut>>,
    dst_scale: Option<f32>,
) -> Box<[[T; 4]]>
where
    T: MetalTfLutValue,
{
    let size = match config {
        LutConfig::Curve(v) => v.len(),
        LutConfig::Size(v) => v,
    };
    let mut res = vec![[T::ZERO; 4]; size].into_boxed_slice();
    if size == 0 {
        return res;
    }
    let dst_mul = dst_scale.unwrap_or(1.0);
    if let Some(post_lut) = post_lut
        && post_lut.gamma_lut.len() > 0
    {
        fn fill_res<T: MetalTfLutValue>(
            res: &mut [[T; 4]],
            mul: f32,
            post_lut: &BackendGammaLut,
            size: usize,
            v: impl Fn(usize) -> f32,
        ) {
            let maxf = (post_lut.gamma_lut.len() - 1) as f32;
            let maxf = maxf.clamp(0.0, f32::MAX_SAFE_INT as f32);
            #[expect(clippy::needless_range_loop)]
            for i in 0..size {
                let v = v(i);
                let v = v.clamp(0.0, 1.0);
                let v = v * maxf;
                let lo = v.floor();
                let hi = v.ceil();
                let alpha = v - lo;
                let beta = 1.0 - alpha;
                let lo = post_lut.gamma_lut[lo as usize];
                let hi = post_lut.gamma_lut[hi as usize];
                macro_rules! mix {
                    ($channel:expr) => {
                        T::from_f32_raw(
                            (beta * lo[$channel] as f32 + alpha * hi[$channel] as f32) * mul,
                        )
                    };
                }
                res[i] = [mix!(0), mix!(1), mix!(2), T::ZERO];
            }
        }
        match config {
            LutConfig::Curve(c) => {
                fill_res::<T>(&mut res, dst_mul, post_lut, size, |v| c[v]);
            }
            LutConfig::Size(_) => {
                let size_maxf = (size - 1) as f32;
                fill_res::<T>(&mut res, dst_mul, post_lut, size, |v| v as f32 / size_maxf);
            }
        }
    } else {
        fn fill_res<T: MetalTfLutValue>(
            res: &mut [[T; 4]],
            mul: f32,
            size: usize,
            v: impl Fn(usize) -> f32,
        ) {
            #[expect(clippy::needless_range_loop)]
            for i in 0..size {
                let v = v(i);
                let v = v * mul;
                let v = T::from_f32(v);
                res[i] = [v, v, v, T::ZERO];
            }
        }
        match config {
            LutConfig::Curve(c) => fill_res::<T>(&mut res, dst_mul, size, |v| c[v]),
            LutConfig::Size(_) => {
                let size_maxf = (size - 1) as f32;
                fill_res::<T>(&mut res, dst_mul, size, |v| v as f32 / size_maxf);
            }
        }
    }
    res
}
