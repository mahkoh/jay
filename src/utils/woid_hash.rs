use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::copyhashmap::LockableRandomState;
use crate::utils::windows::WindowsExt;
use hashbrown::HashMap;
use hashbrown::HashSet;
use rand::random;
use std::hash::BuildHasher;
use std::hash::Hasher;

#[derive(Copy, Clone, Debug)]
pub struct WoidBuildHasher {
    mul: u64,
}

impl Default for WoidBuildHasher {
    #[inline]
    fn default() -> Self {
        Self {
            mul: random::<u64>() | 1,
        }
    }
}

impl BuildHasher for WoidBuildHasher {
    type Hasher = WoidHasher;

    #[inline]
    fn build_hasher(&self) -> Self::Hasher {
        WoidHasher {
            hash: 0,
            mul: self.mul,
        }
    }
}

impl LockableRandomState for WoidBuildHasher {
    const LOCKED_STATE: Self = WoidBuildHasher {
        mul: 0xf1357aea2e62a9c5,
    };
}

#[expect(unused)]
pub type WoidHashSet<T> = HashSet<T, WoidBuildHasher>;

#[expect(unused)]
pub type WoidHashMap<K, V> = HashMap<K, V, WoidBuildHasher>;

pub type WoidCopyHashMap<K, V> = CopyHashMap<K, V, WoidBuildHasher>;

pub struct WoidHasher {
    hash: u64,
    mul: u64,
}

impl Hasher for WoidHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.hash.rotate_left(26)
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for &chunk in bytes.array_chunks_ext::<8>() {
            self.write_u64(u64::from_ne_bytes(chunk));
        }
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.write_u64(i as u64);
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.write_u64(i as u64);
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.write_u64(i as u64);
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.hash = self.hash.wrapping_add(i).wrapping_mul(self.mul);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.write_u64(i as u64);
    }

    #[inline]
    fn write_i8(&mut self, i: i8) {
        self.write_u64(i as u64);
    }

    #[inline]
    fn write_i16(&mut self, i: i16) {
        self.write_u64(i as u64);
    }

    #[inline]
    fn write_i32(&mut self, i: i32) {
        self.write_u64(i as u64);
    }

    #[inline]
    fn write_i64(&mut self, i: i64) {
        self.write_u64(i as u64);
    }

    #[inline]
    fn write_isize(&mut self, i: isize) {
        self.write_u64(i as u64);
    }
}
