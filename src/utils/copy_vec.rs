use crate::utils::markers::JayClone;
use crate::utils::ptr_ext::MutPtrExt;
use crate::utils::ptr_ext::PtrExt;
use derivative::Derivative;
use std::cell::UnsafeCell;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::mem;
use std::ops::Deref;
use std::ops::DerefMut;

#[derive(Derivative)]
#[derivative(Default(bound = ""))]
pub struct CopyVec<V> {
    map: UnsafeCell<Vec<V>>,
}

impl<V> Debug for CopyVec<V> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.map.fmt(f)
    }
}

impl<V> CopyVec<V> {
    #[inline(always)]
    unsafe fn get_map(&self) -> &Vec<V> {
        unsafe { self.map.get().deref() }
    }

    #[inline(always)]
    unsafe fn get_map_mut(&self) -> &mut Vec<V> {
        unsafe { self.map.get().deref_mut() }
    }

    #[expect(unused)]
    pub fn set(&self, idx: usize, v: V) -> V
    where
        V: JayClone + Default,
    {
        let mut map = unsafe { self.get_map_mut() };
        if idx >= map.len() {
            map.reserve(idx - map.len() + 1);
            let def = V::default();
            map = unsafe { self.get_map_mut() };
            if idx >= map.len() {
                for _ in map.len()..idx {
                    map.push(def.clone());
                }
                map.push(def);
            }
        }
        mem::replace(&mut map[idx], v)
    }

    #[expect(unused)]
    pub fn get(&self, idx: usize) -> Option<V>
    where
        V: JayClone,
    {
        let map = unsafe { self.get_map() };
        if idx >= map.len() {
            return None;
        }
        Some(map[idx].clone())
    }

    #[expect(unused)]
    pub fn clear(&self) -> Vec<V> {
        unsafe { mem::take(self.get_map_mut()) }
    }

    #[expect(unused)]
    pub fn is_empty(&self) -> bool {
        unsafe { self.get_map().is_empty() }
    }

    #[expect(unused)]
    pub fn is_not_empty(&self) -> bool {
        !self.is_empty()
    }

    #[expect(unused)]
    pub fn len(&self) -> usize {
        unsafe { self.get_map().len() }
    }

    #[expect(unused)]
    pub fn lock(&self) -> Locked<'_, V> {
        let map = unsafe { self.get_map_mut() };
        Locked {
            copy: self,
            vec: mem::take(map),
        }
    }
}

pub struct Locked<'a, V> {
    copy: &'a CopyVec<V>,
    vec: Vec<V>,
}

impl<V> Drop for Locked<'_, V> {
    fn drop(&mut self) {
        let map = unsafe { self.copy.get_map_mut() };
        mem::swap(map, &mut self.vec);
    }
}

impl<V> Deref for Locked<'_, V> {
    type Target = Vec<V>;

    fn deref(&self) -> &Self::Target {
        &self.vec
    }
}

impl<V> DerefMut for Locked<'_, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.vec
    }
}
