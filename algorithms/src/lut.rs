pub use generated::*;

#[rustfmt::skip]
mod generated;

#[inline(never)]
fn fill(res: &mut [f32], mut src_mul: f32, dst_mul: f32, tf: impl Fn(f32) -> f32 + Copy) {
    src_mul /= (res.len() - 1) as f32;
    #[expect(clippy::needless_range_loop)]
    for i in 0..res.len() {
        let v = i as f32 * src_mul;
        let v = tf(v);
        let v = v * dst_mul;
        res[i] = v;
    }
}

#[inline(never)]
pub fn fill_lut_3d(m: [[f32; 4]; 4], size: usize) -> Vec<[u32; 4]> {
    let mut out = vec![[0u32; 4]; size * size * size];
    let mul = 1.0 / (size - 1) as f32;
    let idx = 0;
    #[rustfmt::skip]
    let v = [
        m[0][3],
        m[1][3],
        m[2][3],
    ];
    for b in 0..size {
        let bf = b as f32 * mul;
        let idx = idx * size + b;
        let v = [
            v[0] + m[0][2] * bf,
            v[1] + m[1][2] * bf,
            v[2] + m[2][2] * bf,
        ];
        for g in 0..size {
            let gf = g as f32 * mul;
            let idx = idx * size + g;
            let v = [
                v[0] + m[0][1] * gf,
                v[1] + m[1][1] * gf,
                v[2] + m[2][1] * gf,
            ];
            for r in 0..size {
                let rf = r as f32 * mul;
                let idx = idx * size + r;
                let v = [
                    v[0] + m[0][0] * rf,
                    v[1] + m[1][0] * rf,
                    v[2] + m[2][0] * rf,
                ];
                out[idx] = [
                    (v[0] as f64 * u32::MAX as f64) as u32,
                    (v[1] as f64 * u32::MAX as f64) as u32,
                    (v[2] as f64 * u32::MAX as f64) as u32,
                    0,
                ];
            }
        }
    }
    out
}
