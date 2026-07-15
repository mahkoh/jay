use {
    crate::utils::{
        markers::JayHash,
        numcell::NumCell,
        object_registry::{ObjectRegistryCache, RegisteredObject},
    },
    rand::{RngExt, prelude::SmallRng},
    std::{mem, ptr, rc::Rc},
};

pub trait Cache<K, V>: Sized
where
    K: JayHash,
{
    type Serial: Copy + Default + Sized;
    fn serial(&mut self) -> Self::Serial;
    fn insert(
        &mut self,
        v: &Rc<RegisteredObject<K, V, Self>>,
    ) -> Option<Rc<RegisteredObject<K, V, Self>>>;
    fn clear(&mut self) -> impl Sized;
}

impl<K, V, C> ObjectRegistryCache<K, V> for C
where
    K: JayHash,
    C: Cache<K, V>,
{
}

pub struct ObjectRegistryNoCache;

impl<K, V> Cache<K, V> for ObjectRegistryNoCache
where
    K: JayHash,
{
    type Serial = ();

    fn serial(&mut self) -> Self::Serial {}

    fn insert(
        &mut self,
        _v: &Rc<RegisteredObject<K, V, Self>>,
    ) -> Option<Rc<RegisteredObject<K, V, Self>>> {
        None
    }

    fn clear(&mut self) -> impl Sized {}
}

pub struct ObjectRegistryRandomCache<K, V>
where
    K: JayHash,
{
    map: Box<[Option<Rc<RegisteredObject<K, V, Self>>>]>,
    rng: SmallRng,
    serial: NumCell<u64>,
}

impl<K, V> ObjectRegistryRandomCache<K, V>
where
    K: JayHash,
{
    pub(super) fn new(size: usize) -> Self {
        Self {
            map: Self::create_map(size),
            rng: rand::make_rng(),
            serial: Default::default(),
        }
    }

    fn create_map(size: usize) -> Box<[Option<Rc<RegisteredObject<K, V, Self>>>]> {
        let size = size.next_power_of_two().max(16);
        vec![None; size].into_boxed_slice()
    }
}

impl<K, V> Cache<K, V> for ObjectRegistryRandomCache<K, V>
where
    K: JayHash,
{
    type Serial = u64;

    fn serial(&mut self) -> Self::Serial {
        self.serial.add_fetch(1)
    }

    fn insert(
        &mut self,
        value: &Rc<RegisteredObject<K, V, Self>>,
    ) -> Option<Rc<RegisteredObject<K, V, Self>>> {
        let value = value.clone();
        value.serial.set(self.serial());
        let r1 = self.rng.random::<u64>() as usize;
        let r2 = self.rng.random::<u64>() as usize;
        let offsets = [r1, r2, r2.wrapping_add(1)];
        let entries = unsafe {
            let mask = self.map.len() - 1;
            let ptr = self.map.as_mut_ptr();
            offsets.map(|idx| ptr.add(idx & mask))
        };
        let mut min_entry = ptr::null_mut();
        let mut min_serial = u64::MAX;
        for entry in entries {
            match unsafe { &mut *entry } {
                e @ None => {
                    *e = Some(value);
                    return None;
                }
                Some(e) => {
                    let serial = e.serial.get();
                    if serial < min_serial {
                        min_entry = e;
                        min_serial = serial;
                    }
                }
            }
        }
        let old = unsafe { mem::replace(&mut *min_entry, value) };
        Some(old)
    }

    fn clear(&mut self) -> impl Sized {
        let new = Self::create_map(self.map.len());
        mem::replace(&mut self.map, new)
    }
}
