#[cfg(test)]
pub mod tests;

#[cfg_attr(not(test), expect(dead_code))]
pub trait FloatExt {
    const MAX_SAFE_INT: u64;
}

macro_rules! imp {
    ($ty:ty) => {
        impl FloatExt for $ty {
            const MAX_SAFE_INT: u64 = 1 << <$ty>::MANTISSA_DIGITS;
        }
    };
}

imp!(f32);
imp!(f64);
