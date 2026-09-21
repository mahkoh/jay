use crate::utils::ordered_float::F32;
use crate::utils::static_text::StaticText;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
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

impl StrFmt for Eotf {
    fn str_fmt(&self, dst: &mut String, ctx: &StrCtx<'_>) {
        let name = self.text();
        match self.arg() {
            None => name.str_fmt(dst, ctx),
            Some(arg) => {
                let mut buf = zmij::Buffer::new();
                let arg = buf.format(arg);
                let mut text = String::with_capacity(name.len() + 1 + arg.len() + 1);
                text.push_str(name);
                text.push_str("(");
                text.push_str(arg);
                text.push_str(")");
                text.str_fmt(dst, ctx);
            }
        }
    }
}

impl StaticText for Eotf {
    fn text(&self) -> &'static str {
        match self {
            Eotf::Linear => "linear",
            Eotf::St2084Pq => "st2084_pq",
            Eotf::Bt1886(_) => "bt1886",
            Eotf::Gamma22 => "gamma22",
            Eotf::Gamma24 => "gamma24",
            Eotf::Gamma28 => "gamma28",
            Eotf::St240 => "st240",
            Eotf::Log100 => "log100",
            Eotf::Log316 => "log316",
            Eotf::St428 => "st428",
            Eotf::Pow(_) => "pow",
            Eotf::CompoundPower24 => "compound_power24",
        }
    }
}

impl Eotf {
    pub fn arg(self) -> Option<f32> {
        match self {
            Eotf::Bt1886(p) => Some(p.0),
            Eotf::Pow(p) => Some(p.eotf_f32()),
            _ => None,
        }
    }

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
