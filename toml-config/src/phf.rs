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

const M: u64 = 17099814477566751079;
const K: u64 = 0x9e37_79b9_7f4a_7c15;

#[inline]
fn fmix32(mut a: u32) -> u32 {
    a ^= a >> 16;
    a = a.wrapping_mul(0x7feb352d);
    a ^= a >> 15;
    a = a.wrapping_mul(0x846ca68b);
    a ^= a >> 16;
    a
}

#[inline]
fn fmix64(mut b: u64) -> u64 {
    b ^= b >> 33;
    b = b.wrapping_mul(0xff51afd7ed558ccd);
    b ^= b >> 33;
    b = b.wrapping_mul(0xc4ceb9fe1a85ec53);
    b ^= b >> 33;
    b
}

#[inline]
fn finish(acc: u64) -> (u32, u64) {
    let lower = fmix64(acc);
    let upper = fmix32((lower >> 32) as u32 ^ lower as u32);
    (upper, lower)
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

#[inline]
fn tail(x: &[u8]) -> u64 {
    match x.len() {
        0 => 0,
        1 => x[0] as u64,
        2..=3 => {
            let lo = u16::from_le_bytes([x[0], x[1]]) as u64;
            let hi = u16::from_le_bytes([x[x.len() - 2], x[x.len() - 1]]) as u64;
            lo | (hi << 16)
        }
        _ => {
            let n = x.len();
            let lo = u32::from_le_bytes([x[0], x[1], x[2], x[3]]) as u64;
            let hi = u32::from_le_bytes([x[n - 4], x[n - 3], x[n - 2], x[n - 1]]) as u64;
            lo | (hi << 32)
        }
    }
}

impl PhfHash for [u8] {
    #[inline]
    fn phf_hash(&self, key: u64) -> (u32, u64) {
        let mut acc = key ^ (self.len() as u64).wrapping_mul(K);
        let (chunks, rest) = self.as_chunks::<8>();
        for chunk in chunks {
            acc = (acc ^ u64::from_le_bytes(*chunk)).wrapping_mul(M);
        }
        finish((acc ^ tail(rest)).wrapping_mul(M))
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
        finish((key ^ *self as u64).wrapping_mul(M))
    }
}

impl PhfHash for char {
    #[inline]
    fn phf_hash(&self, key: u64) -> (u32, u64) {
        (*self as u32).phf_hash(key)
    }
}
