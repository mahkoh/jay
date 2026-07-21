#![expect(clippy::excessive_precision)]

#[derive(Copy, Clone)]
pub enum AlgoEotf {
    Linear,
    St2084Pq,
    Bt1886(f32),
    Gamma22,
    Gamma24,
    Gamma28,
    St240,
    Log100,
    Log316,
    St428,
    Pow(f32),
    CompoundPower24,
}

pub fn bt1886_eotf_args<T>(c: f32) -> [f32; 4] {
    let gamma = 1.0 / 2.4;
    let a1 = 1.0 / (1.0 - c);
    let a2 = 1.0 - c.powf(gamma);
    let a3 = c.powf(gamma);
    let a4 = c;
    [a1, a2, a3, a4]
}

pub fn bt1886_inv_eotf_args<T>(c: f32) -> [f32; 4] {
    let gamma = 1.0 / 2.4;
    let a1 = 1.0 / (1.0 - c.powf(gamma));
    let a2 = 1.0 - c;
    let a3 = c;
    let a4 = c.powf(gamma);
    [a1, a2, a3, a4]
}

pub mod eotfs {
    use crate::tf::bt1886_eotf_args;

    #[inline(always)]
    pub fn linear<T>(c: f32) -> f32 {
        c
    }
    pub fn st2084_pq<T>(c: f32) -> f32 {
        let cp = c.powf(1.0 / 78.84375);
        let num = (cp - 0.8359375).max(0.0);
        let den = 18.8515625 - 18.6875 * cp;
        (num / den).powf(1.0 / 0.1593017578125)
    }
    pub fn st240<T>(c: f32) -> f32 {
        if c < 0.0913 {
            c / 4.0
        } else {
            ((c + 0.1115) / 1.1115).powf(1.0 / 0.45)
        }
    }
    pub fn log100<T>(c: f32) -> f32 {
        10.0f32.powf(2.0 * (c - 1.0))
    }
    pub fn log316<T>(c: f32) -> f32 {
        10.0f32.powf(2.5 * (c - 1.0))
    }
    pub fn st428<T>(c: f32) -> f32 {
        c.powf(2.6) * 52.37 / 48.0
    }
    pub fn gamma22<T>(c: f32) -> f32 {
        c.signum() * c.abs().powf(2.2)
    }
    pub fn gamma24<T>(c: f32) -> f32 {
        c.signum() * c.abs().powf(2.4)
    }
    pub fn gamma28<T>(c: f32) -> f32 {
        c.signum() * c.abs().powf(2.8)
    }
    pub fn compound_power_2_4<T>(c: f32) -> f32 {
        if c < 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    pub fn pow<T>(e: f32) -> impl Fn(f32) -> f32 + Copy {
        move |c: f32| -> f32 { c.signum() * c.abs().powf(e) }
    }
    pub fn bt1886<T>(c: f32) -> impl Fn(f32) -> f32 + Copy {
        let [a1, a2, a3, a4] = bt1886_eotf_args::<T>(c);
        move |c: f32| -> f32 { a1 * ((a2 * c + a3).powf(2.4) - a4) }
    }
}

pub mod inv_eotfs {
    use crate::tf::bt1886_inv_eotf_args;

    #[inline(always)]
    pub fn linear<T>(c: f32) -> f32 {
        c
    }
    pub fn st2084_pq<T>(c: f32) -> f32 {
        let c = c.clamp(0.0, 1.0);
        let num = 0.8359375 + 18.8515625 * c.powf(0.1593017578125);
        let den = 1.0 + 18.6875 * c.powf(0.1593017578125);
        (num / den).powf(78.84375)
    }
    pub fn st240<T>(c: f32) -> f32 {
        if c < 0.0228 {
            4.0 * c
        } else {
            1.1115 * c.powf(0.45) - 0.1115
        }
    }
    pub fn log100<T>(c: f32) -> f32 {
        let c = c.clamp(0.0, 1.0);
        if c < 0.01 { 0.0 } else { 1.0 + c.log10() / 2.0 }
    }
    pub fn log316<T>(c: f32) -> f32 {
        let c = c.clamp(0.0, 1.0);
        if c < 10.0f32.sqrt() / 1000.0 {
            0.0
        } else {
            1.0 + c.log10() / 2.5
        }
    }
    pub fn st428<T>(c: f32) -> f32 {
        (48.0 * c / 52.37).powf(1.0 / 2.6)
    }
    pub fn gamma22<T>(c: f32) -> f32 {
        c.signum() * c.abs().powf(1.0 / 2.2)
    }
    pub fn gamma24<T>(c: f32) -> f32 {
        c.signum() * c.abs().powf(1.0 / 2.4)
    }
    pub fn gamma28<T>(c: f32) -> f32 {
        c.signum() * c.abs().powf(1.0 / 2.8)
    }
    pub fn compound_power_2_4<T>(c: f32) -> f32 {
        if c < 0.0031308 {
            12.92 * c
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        }
    }
    pub fn bt1886<T>(c: f32) -> impl Fn(f32) -> f32 + Copy {
        let [a1, a2, a3, a4] = bt1886_inv_eotf_args::<T>(c);
        move |c: f32| -> f32 { a1 * ((a2 * c + a3).powf(1.0 / 2.4) - a4) }
    }
    pub fn pow<T>(e: f32) -> impl Fn(f32) -> f32 + Copy {
        let e = 1.0 / e;
        move |c: f32| -> f32 { c.signum() * c.abs().powf(e) }
    }
}
