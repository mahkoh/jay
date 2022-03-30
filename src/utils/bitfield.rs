#![allow(dead_code)]

use std::mem;

const SEG_SIZE: usize = 8 * mem::size_of::<usize>();

#[derive(Default)]
pub struct Bitfield {
    vals: Vec<usize>,
}

impl Bitfield {
    pub fn take(&mut self, val: u32) {
        let idx = val as usize / SEG_SIZE;
        let pos = val as usize % SEG_SIZE;
        while self.vals.len() <= idx {
            self.vals.push(0);
        }
        self.vals[idx] &= !(1 << pos);
    }

    pub fn acquire(&mut self) -> u32 {
        for (idx, n) in self.vals.iter_mut().enumerate() {
            if *n != 0 {
                let pos = n.trailing_zeros();
                *n &= !(1 << pos);
                return (idx * SEG_SIZE) as u32 + pos;
            }
        }
        self.vals.push(!1);
        ((self.vals.len() - 1) * SEG_SIZE) as u32
    }

    pub fn release(&mut self, val: u32) {
        let idx = val as usize / SEG_SIZE;
        let pos = val as usize % SEG_SIZE;
        self.vals[idx] |= 1 << pos;
    }
}
