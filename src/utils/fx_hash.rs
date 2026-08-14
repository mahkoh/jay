use rustc_hash::FxHasher;
use std::hash::BuildHasher;

#[derive(Copy, Clone, Debug, Default)]
pub struct FxBuildHasher;

impl BuildHasher for FxBuildHasher {
    type Hasher = FxHasher;

    #[inline]
    fn build_hasher(&self) -> Self::Hasher {
        FxHasher::default()
    }
}
