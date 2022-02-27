use uapi::{Packed, Pod};

#[repr(C, align(8))]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct AlignedI64(pub i64);

unsafe impl Pod for AlignedI64 {}
unsafe impl Packed for AlignedI64 {}

#[repr(C, align(8))]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct AlignedU64(pub u64);

unsafe impl Pod for AlignedU64 {}
unsafe impl Packed for AlignedU64 {}

#[repr(C, align(8))]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct AlignedF64(pub f64);

unsafe impl Pod for AlignedF64 {}
unsafe impl Packed for AlignedF64 {}
