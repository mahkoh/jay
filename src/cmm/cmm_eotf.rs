use crate::utils::ordered_float::F32;
use jay_algorithms::tf::AlgoEotf;
use jay_proc::jay_hash;

#[jay_hash]
#[derive(Copy, Clone, Debug, Eq)]
pub enum Eotf {
    Linear,
    St2084Pq,
    Bt1886(F32),
    Gamma22,
    Gamma24,
    Gamma28,
    St240,
    Log100,
    Log316,
    St428,
    Pow(EotfPow),
    CompoundPower24,
}

const MUL: u32 = 10_000;
const MUL_F32: f32 = MUL as f32;

#[jay_hash]
#[derive(Copy, Clone, Debug, Eq, Ord, PartialOrd)]
pub struct EotfPow(pub u32);

impl EotfPow {
    pub const MIN: Self = Self(10_000);
    pub const LINEAR: Self = Self(10_000);
    pub const GAMMA22: Self = Self(22_000);
    pub const GAMMA24: Self = Self(24_000);
    pub const GAMMA28: Self = Self(28_000);
    pub const MAX: Self = Self(100_000);

    pub fn eotf_f32(self) -> f32 {
        self.0 as f32 / MUL_F32
    }

    pub fn inv_eotf_f32(self) -> f32 {
        MUL_F32 / self.0 as f32
    }
}

impl Eotf {
    pub fn to_algo(self) -> AlgoEotf {
        match self {
            Eotf::Linear => AlgoEotf::Linear,
            Eotf::St2084Pq => AlgoEotf::St2084Pq,
            Eotf::Bt1886(p) => AlgoEotf::Bt1886(p.0),
            Eotf::Gamma22 => AlgoEotf::Gamma22,
            Eotf::Gamma24 => AlgoEotf::Gamma24,
            Eotf::Gamma28 => AlgoEotf::Gamma28,
            Eotf::St240 => AlgoEotf::St240,
            Eotf::Log100 => AlgoEotf::Log100,
            Eotf::Log316 => AlgoEotf::Log316,
            Eotf::St428 => AlgoEotf::St428,
            Eotf::Pow(p) => AlgoEotf::Pow(p.eotf_f32()),
            Eotf::CompoundPower24 => AlgoEotf::CompoundPower24,
        }
    }
}
