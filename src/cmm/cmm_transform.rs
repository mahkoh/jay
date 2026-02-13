use {
    crate::{
        cmm::{cmm_eotf::Eotf, cmm_primaries::Primaries},
        theme::Color,
        utils::ordered_float::F64,
    },
    std::{
        fmt,
        fmt::{Debug, Formatter},
        hash::{Hash, Hasher},
        marker::PhantomData,
        ops::{Mul, MulAssign},
    },
};

pub struct ColorMatrix<To = Local, From = Local>(pub [[F64; 4]; 3], PhantomData<(To, From)>);

#[derive(Copy, Clone)]
pub struct Local;
#[derive(Copy, Clone)]
pub struct Xyz;
#[derive(Copy, Clone)]
pub struct Bradford;

impl<T, U> Copy for ColorMatrix<T, U> {}

impl<T, U> Clone for ColorMatrix<T, U> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T, U> PartialEq<Self> for ColorMatrix<T, U> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T, U> Eq for ColorMatrix<T, U> {}

impl<T, U> Hash for ColorMatrix<T, U> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<T, U> Debug for ColorMatrix<T, U> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ColorMatrix")
            .field(&format_matrix(&self.0))
            .finish()
    }
}

fn format_matrix<'a>(m: &'a [[F64; 4]; 3]) -> impl Debug + use<'a> {
    fmt::from_fn(move |f| {
        let iter = m
            .iter()
            .copied()
            .chain(Some([F64(0.0), F64(0.0), F64(0.0), F64(1.0)]))
            .enumerate();
        if f.alternate() {
            for (idx, row) in iter {
                if idx > 0 {
                    f.write_str("\n")?;
                }
                write!(
                    f,
                    "{:7.4} {:7.4} {:7.4} {:7.4}",
                    row[0], row[1], row[2], row[3]
                )?;
            }
        } else {
            f.write_str("[")?;
            for (idx, row) in iter {
                if idx > 0 {
                    f.write_str(", ")?;
                }
                write!(
                    f,
                    "[{:.4}, {:.4}, {:.4}, {:.4}]",
                    row[0], row[1], row[2], row[3]
                )?;
            }
            f.write_str("]")?;
        }
        Ok(())
    })
}

impl<T, U, V> Mul<ColorMatrix<U, T>> for ColorMatrix<V, U> {
    type Output = ColorMatrix<V, T>;

    fn mul(self, rhs: ColorMatrix<U, T>) -> Self::Output {
        let a = &self.0;
        let b = &rhs.0;
        macro_rules! mul {
            ($ar:expr, $bc:expr) => {
                a[$ar][0] * b[0][$bc] + a[$ar][1] * b[1][$bc] + a[$ar][2] * b[2][$bc]
            };
        }
        let m = [
            [mul!(0, 0), mul!(0, 1), mul!(0, 2), mul!(0, 3) + a[0][3]],
            [mul!(1, 0), mul!(1, 1), mul!(1, 2), mul!(1, 3) + a[1][3]],
            [mul!(2, 0), mul!(2, 1), mul!(2, 2), mul!(2, 3) + a[2][3]],
        ];
        ColorMatrix(m, PhantomData)
    }
}

impl<U, V> MulAssign<ColorMatrix<U, U>> for ColorMatrix<V, U> {
    fn mul_assign(&mut self, rhs: ColorMatrix<U, U>) {
        *self = *self * rhs;
    }
}

impl<T, U> Mul<[f64; 3]> for ColorMatrix<T, U> {
    type Output = [f64; 3];

    fn mul(self, rhs: [f64; 3]) -> Self::Output {
        let a = &self.0;
        macro_rules! mul {
            ($ar:expr) => {
                a[$ar][0].0 * rhs[0] + a[$ar][1].0 * rhs[1] + a[$ar][2].0 * rhs[2]
            };
        }
        [mul!(0), mul!(1), mul!(2)]
    }
}

impl<T, U> Mul<Color> for ColorMatrix<T, U> {
    type Output = Color;

    fn mul(self, rhs: Color) -> Self::Output {
        let mut rgba = rhs.to_array(Eotf::Linear);
        let a = rgba[3];
        if a < 1.0 && a > 0.0 {
            for c in &mut rgba[..3] {
                *c /= a;
            }
        }
        let [r, g, b] = self * [rgba[0] as f64, rgba[1] as f64, rgba[2] as f64];
        let mut color = Color::new(Eotf::Linear, r as f32, g as f32, b as f32);
        if a < 1.0 {
            color = color * a;
        }
        color
    }
}

impl<T, U> ColorMatrix<T, U> {
    pub const fn new(m: [[f64; 4]; 3]) -> Self {
        let m = [
            [F64(m[0][0]), F64(m[0][1]), F64(m[0][2]), F64(m[0][3])],
            [F64(m[1][0]), F64(m[1][1]), F64(m[1][2]), F64(m[1][3])],
            [F64(m[2][0]), F64(m[2][1]), F64(m[2][2]), F64(m[2][3])],
        ];
        Self(m, PhantomData)
    }

    pub const fn to_f32(&self) -> [[f32; 4]; 4] {
        let m = &self.0;
        macro_rules! map {
            ($r:expr, $c:expr) => {
                m[$r][$c].0 as f32
            };
        }
        [
            [map!(0, 0), map!(0, 1), map!(0, 2), map!(0, 3)],
            [map!(1, 0), map!(1, 1), map!(1, 2), map!(1, 3)],
            [map!(2, 0), map!(2, 1), map!(2, 2), map!(2, 3)],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }
}

impl ColorMatrix<Bradford, Xyz> {
    const BFD: Self = Self::new([
        [0.8951, 0.2664, -0.1614, 0.0],
        [-0.7502, 1.7135, 0.0367, 0.0],
        [0.0389, -0.0685, 1.0296, 0.0],
    ]);
}

impl ColorMatrix<Xyz, Bradford> {
    const BFD_INV: Self = Self::new([
        [0.9870, -0.1471, 0.1600, 0.0],
        [0.4323, 0.5184, 0.0493, 0.0],
        [-0.0085, 0.04, 0.9685, 0.0],
    ]);
}

#[expect(non_snake_case)]
pub fn bradford_adjustment(w_from: (F64, F64), w_to: (F64, F64)) -> ColorMatrix<Xyz, Xyz> {
    let (F64(x_from), F64(y_from)) = w_from;
    let (F64(x_to), F64(y_to)) = w_to;
    let X_from = x_from / y_from;
    let Z_from = (1.0 - x_from - y_from) / y_from;
    let X_to = x_to / y_to;
    let Z_to = (1.0 - x_to - y_to) / y_to;
    let [R_from, G_from, B_from] = ColorMatrix::BFD * [X_from, 1.0, Z_from];
    let [R_to, G_to, B_to] = ColorMatrix::BFD * [X_to, 1.0, Z_to];
    let adj = ColorMatrix::new([
        [R_to / R_from, 0.0, 0.0, 0.0],
        [0.0, G_to / G_from, 0.0, 0.0],
        [0.0, 0.0, B_to / B_from, 0.0],
    ]);
    ColorMatrix::BFD_INV * adj * ColorMatrix::BFD
}

impl Primaries {
    #[expect(non_snake_case)]
    pub const fn matrices(&self) -> (ColorMatrix<Xyz, Local>, ColorMatrix<Local, Xyz>) {
        let (F64(xw), F64(yw)) = self.wp;
        let Xw = xw / yw;
        let Zw = (1.0 - xw - yw) / yw;
        let (F64(xr), F64(yr)) = self.r;
        let (F64(xg), F64(yg)) = self.g;
        let (F64(xb), F64(yb)) = self.b;
        let zr = 1.0 - xr - yr;
        let zg = 1.0 - xg - yg;
        let zb = 1.0 - xb - yb;
        let srx = yg * zb - zg * yb;
        let sry = zg * xb - xg * zb;
        let srz = xg * yb - yg * xb;
        let sgx = zr * yb - yr * zb;
        let sgz = yr * xb - xr * yb;
        let sgy = xr * zb - zr * xb;
        let sbx = yr * zg - zr * yg;
        let sby = zr * xg - xr * zg;
        let sbz = xr * yg - yr * xg;
        let det = srz + sgz + sbz;
        let sr = srx * Xw + sry + srz * Zw;
        let sg = sgx * Xw + sgy + sgz * Zw;
        let sb = sbx * Xw + sby + sbz * Zw;
        let det_inv = 1.0 / det;
        let sr_inv = 1.0 / sr;
        let sg_inv = 1.0 / sg;
        let sb_inv = 1.0 / sb;
        let srp = sr * det_inv;
        let sgp = sg * det_inv;
        let sbp = sb * det_inv;
        let XYZ_from_local = [
            [srp * xr, sgp * xg, sbp * xb, 0.0],
            [srp * yr, sgp * yg, sbp * yb, 0.0],
            [srp * zr, sgp * zg, sbp * zb, 0.0],
        ];
        let local_from_XYZ = [
            [srx * sr_inv, sry * sr_inv, srz * sr_inv, 0.0],
            [sgx * sg_inv, sgy * sg_inv, sgz * sg_inv, 0.0],
            [sbx * sb_inv, sby * sb_inv, sbz * sb_inv, 0.0],
        ];
        (
            ColorMatrix::new(XYZ_from_local),
            ColorMatrix::new(local_from_XYZ),
        )
    }
}
