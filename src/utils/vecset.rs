use std::ops::Deref;

#[derive(Debug)]
pub struct VecSet<T> {
    vec: Vec<T>,
}

impl<T> Default for VecSet<T> {
    fn default() -> Self {
        Self { vec: vec![] }
    }
}

impl<T> Deref for VecSet<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.vec
    }
}

impl<T> VecSet<T> {
    #[expect(dead_code)]
    pub fn clear(&mut self) {
        self.vec.clear();
    }

    pub fn drain<RANGE>(&mut self, range: RANGE) -> Self
    where
        RANGE: std::ops::RangeBounds<usize>,
    {
        Self {
            vec: self.vec.drain(range).collect(),
        }
    }
}

impl<T: PartialEq> VecSet<T> {
    pub fn insert(&mut self, val: T) -> bool {
        if self.vec.contains(&val) {
            return false;
        }
        self.vec.push(val);
        true
    }

    #[allow(dead_code)]
    pub fn extend(&mut self, vec: impl IntoIterator<Item = T>) -> bool {
        let mut res = false;
        for item in vec {
            res |= self.insert(item);
        }
        res
    }

    pub fn remove(&mut self, val: &T) -> bool {
        for i in 0..self.vec.len() {
            if self.vec[i] == *val {
                self.vec.swap_remove(i);
                return true;
            }
        }
        false
    }
}
