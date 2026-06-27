use core::num::Wrapping;

#[non_exhaustive]
pub(crate) struct Hashes {
    pub(crate) g: u32,
    pub(crate) f1: u32,
    pub(crate) f2: u32,
}

#[inline]
pub(crate) fn displace(f1: u32, f2: u32, d1: u32, d2: u32) -> u32 {
    (Wrapping(d2) + Wrapping(f1) * Wrapping(d1) + Wrapping(f2)).0
}

#[inline]
pub(crate) fn hash<T>(x: &T, key: u64) -> Hashes
where
    T: ?Sized + PhfHash,
{
    let (upper, lower) = x.phf_hash(key);
    Hashes {
        g: (lower >> 32) as u32,
        f1: lower as u32,
        f2: upper,
    }
}

#[inline]
#[allow(dead_code)]
pub(crate) fn get_unwrapped_index(hashes: &Hashes, disps: &[(u32, u32)]) -> usize {
    let (d1, d2) = disps[(hashes.g % (disps.len() as u32)) as usize];
    displace(hashes.f1, hashes.f2, d1, d2) as usize
}

pub(crate) trait PhfHash {
    fn phf_hash(&self, key: u64) -> (u32, u64);
}

impl<T> PhfHash for &'_ T
where
    T: ?Sized + PhfHash,
{
    #[inline]
    fn phf_hash(&self, key: u64) -> (u32, u64) {
        (**self).phf_hash(key)
    }
}

impl PhfHash for [u8] {
    #[inline]
    fn phf_hash(&self, key: u64) -> (u32, u64) {
        const A: u32 = 2024877429;
        const B: u64 = 17099814477566751079;
        let mut a = key as u32;
        let mut b = key;
        for &x in self {
            a = (a ^ x as u32).wrapping_mul(A);
            b = (b ^ x as u64).wrapping_mul(B);
        }
        (a, b)
    }
}

impl PhfHash for str {
    #[inline]
    fn phf_hash(&self, key: u64) -> (u32, u64) {
        self.as_bytes().phf_hash(key)
    }
}

impl PhfHash for u32 {
    #[inline]
    fn phf_hash(&self, key: u64) -> (u32, u64) {
        const A: u32 = 2024877429;
        const B: u64 = 17099814477566751079;
        let a = (key as u32 ^ *self).wrapping_mul(A);
        let b = (key ^ *self as u64).wrapping_mul(B);
        (a, b)
    }
}

impl PhfHash for char {
    #[inline]
    fn phf_hash(&self, key: u64) -> (u32, u64) {
        (*self as u32).phf_hash(key)
    }
}
