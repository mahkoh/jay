use linearize::Linearize;
use linearize::LinearizeExt;
use linearize::StaticCopyMap;
use num_traits::ConstOne;
use num_traits::ConstZero;
use std::ops::BitAnd;
use std::ops::BitAndAssign;
use std::ops::BitOrAssign;
use std::ops::Shl;
use std::ops::ShlAssign;

pub(super) type MatcherBits = u64;

pub trait Matcher<Op>: Sync {
    fn find(&self, pipeline: &[(Op, bool)], res: &mut [MatcherBits]);
}

pub(super) struct MatcherImpl<I, Op, const NUM_PATHS: usize>
where
    I: Copy,
    Op: Linearize,
{
    pub(super) tt: [StaticCopyMap<Op, I>; NUM_PATHS],
    pub(super) len: [usize; NUM_PATHS],
}

impl<I, Op, const NUM_PATHS: usize> Matcher<Op> for MatcherImpl<I, Op, NUM_PATHS>
where
    I: Copy
        + Eq
        + ConstOne
        + ConstZero
        + BitAndAssign
        + BitAnd<Output = I>
        + ShlAssign<u32>
        + Shl<usize, Output = I>
        + BitOrAssign,
    Op: Copy + Linearize,
    StaticCopyMap<Op, I>: Sync,
{
    fn find(&self, pipeline: &[(Op, bool)], res: &mut [MatcherBits]) {
        const BITS: usize = MatcherBits::BITS as usize;
        let mut states = [I::ONE; NUM_PATHS];
        for &(op, bypass) in pipeline {
            let op = op.linearized();
            #[expect(clippy::needless_range_loop)]
            for i in 0..NUM_PATHS {
                let old = states[i];
                states[i] &= self.tt[i][op];
                states[i] <<= 1;
                if bypass {
                    states[i] |= old;
                }
            }
        }
        macro_rules! fill {
            ($idx:expr, $max_shift:expr) => {
                let mut v = 0;
                for shift in 0..$max_shift {
                    let i = $idx * BITS + shift;
                    v |= ((states[i] & (I::ONE << self.len[i]) != I::ZERO) as u64) << shift;
                }
                res[$idx] = v;
            };
        }
        assert!((NUM_PATHS - 1) / BITS < res.len());
        #[expect(clippy::needless_range_loop)]
        for idx in 0..NUM_PATHS / BITS {
            fill!(idx, BITS);
        }
        if NUM_PATHS % BITS != 0 {
            let idx = NUM_PATHS / BITS;
            fill!(idx, NUM_PATHS % BITS);
        }
    }
}
