use {
    crate::{
        rect::{Rect, Region},
        utils::{
            array,
            ptr_ext::{MutPtrExt, PtrExt},
        },
    },
    jay_algorithms::rect::{
        RectRaw, Tag,
        region::{
            extents, intersect, intersect_tagged, rects_to_bands, rects_to_bands_tagged, subtract,
            union,
        },
    },
    smallvec::SmallVec,
    std::{
        cell::UnsafeCell,
        fmt::{Debug, Formatter},
        mem,
        ops::Deref,
        rc::Rc,
    },
};

thread_local! {
    static EMPTY: Rc<Region> =
        Rc::new(Region {
            rects: Default::default(),
            extents: Default::default(),
        });
}

impl Region {
    pub fn empty() -> Rc<Self> {
        EMPTY.with(|e| e.clone())
    }

    pub fn from_rects(rects: &[Rect]) -> Rc<Self> {
        if rects.is_empty() {
            return Self::empty();
        }
        Rc::new(Self::from_rects2(rects))
    }

    pub fn from_rects2(rects: &[Rect]) -> Self {
        if rects.is_empty() {
            return Self::default();
        }
        if rects.len() == 1 {
            return Self::new2(rects[0]);
        }
        let rects = rects_to_bands(unsafe { mem::transmute::<&[Rect], &[RectRaw]>(rects) });
        Self {
            extents: Rect {
                raw: extents(&rects),
            },
            rects,
        }
    }

    pub fn union(self: &Rc<Self>, other: &Rc<Self>) -> Rc<Self> {
        if self.extents.is_empty() {
            return other.clone();
        }
        if other.extents.is_empty() {
            return self.clone();
        }
        let rects = union(&self.rects, &other.rects);
        Rc::new(Self {
            rects,
            extents: self.extents.union(other.extents),
        })
    }

    pub fn subtract(self: &Rc<Self>, other: &Rc<Self>) -> Rc<Self> {
        if self.extents.is_empty() || other.extents.is_empty() {
            return self.clone();
        }
        let rects = subtract(&self.rects, &other.rects);
        Rc::new(Self {
            extents: Rect {
                raw: extents(&rects),
            },
            rects,
        })
    }

    #[cfg_attr(not(test), expect(dead_code))]
    pub fn intersect(&self, other: &Region) -> Self {
        if self.is_empty() || other.is_empty() {
            return Self::default();
        }
        let rects = intersect(&self.rects, &other.rects);
        Self {
            extents: Rect {
                raw: extents(&rects),
            },
            rects,
        }
    }
}

impl Region<u32> {
    #[cfg_attr(not(test), expect(dead_code))]
    pub fn from_rects_tagged(rects: &[Rect<u32>]) -> Self {
        if rects.is_empty() {
            return Self::default();
        }
        if rects.len() == 1 {
            let mut rect = rects[0];
            rect.raw.tag = rect.raw.tag.constrain();
            return Self::new2(rect);
        }
        let rects = rects_to_bands_tagged(unsafe {
            mem::transmute::<&[Rect<u32>], &[RectRaw<u32>]>(rects)
        });
        Self {
            extents: Rect {
                raw: extents(&rects),
            },
            rects,
        }
    }

    #[cfg_attr(not(test), expect(dead_code))]
    pub fn intersect_tagged(&self, other: &Region) -> Self {
        if self.is_empty() || other.is_empty() {
            return Self::default();
        }
        let rects = intersect_tagged(&self.rects, &other.rects);
        Self {
            extents: Rect {
                raw: extents(&rects),
            },
            rects,
        }
    }
}

impl<T> Region<T>
where
    T: Tag,
{
    pub fn new(rect: Rect<T>) -> Rc<Self> {
        Rc::new(Self::new2(rect))
    }

    pub fn new2(rect: Rect<T>) -> Self {
        let mut rects = SmallVec::new();
        rects.push(rect.raw);
        Self {
            rects,
            extents: rect.untag(),
        }
    }

    #[cfg_attr(not(feature = "it"), expect(dead_code))]
    pub fn extents(&self) -> Rect {
        self.extents
    }

    pub fn rects(&self) -> &[Rect<T>] {
        unsafe { mem::transmute::<&[RectRaw<T>], &[Rect<T>]>(&self.rects[..]) }
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        if !self.extents.contains(x, y) {
            return false;
        }
        for r in self.deref() {
            if r.contains(x, y) {
                return true;
            }
        }
        false
    }
}

impl<T> Deref for Region<T>
where
    T: Tag,
{
    type Target = [Rect<T>];

    fn deref(&self) -> &Self::Target {
        unsafe { mem::transmute::<&[RectRaw<T>], _>(&self.rects) }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum BuilderOp {
    Add,
    Sub,
}

impl Default for BuilderOp {
    fn default() -> Self {
        Self::Add
    }
}

#[derive(Debug)]
pub struct RegionBuilder {
    base: Rc<Region>,
    op: BuilderOp,
    pending: Vec<Rect>,
}

impl Default for RegionBuilder {
    fn default() -> Self {
        Self {
            base: Region::empty(),
            op: Default::default(),
            pending: Default::default(),
        }
    }
}

impl RegionBuilder {
    pub fn add(&mut self, rect: Rect) {
        self.set_op(BuilderOp::Add);
        self.pending.push(rect);
    }

    pub fn sub(&mut self, rect: Rect) {
        self.set_op(BuilderOp::Sub);
        self.pending.push(rect);
    }

    pub fn get(&mut self) -> Rc<Region> {
        self.flush();
        self.base.clone()
    }

    #[expect(dead_code)]
    pub fn clear(&mut self) {
        self.pending.clear();
        self.base = Region::empty();
    }

    fn set_op(&mut self, op: BuilderOp) {
        if self.op != op {
            self.flush();
            self.op = op;
        }
    }

    fn flush(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let region = Region::from_rects(&self.pending);
        self.base = match self.op {
            BuilderOp::Add => self.base.union(&region),
            BuilderOp::Sub => self.base.subtract(&region),
        };
        self.pending.clear();
    }
}

pub struct DamageQueue {
    this: usize,
    datas: Rc<UnsafeCell<Vec<Vec<Rect>>>>,
}

impl Debug for DamageQueue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DamageQueue").finish_non_exhaustive()
    }
}

impl DamageQueue {
    pub fn new<const N: usize>() -> [DamageQueue; N] {
        let datas = Rc::new(UnsafeCell::new(vec![vec!(); N]));
        array::from_fn(|this| DamageQueue {
            this,
            datas: datas.clone(),
        })
    }

    pub fn damage(&self, rects: &[Rect]) {
        let datas = unsafe { self.datas.get().deref_mut() };
        for data in datas {
            data.extend(rects);
        }
    }

    pub fn clear(&self) {
        let data = unsafe { &mut self.datas.get().deref_mut()[self.this] };
        data.clear();
    }

    pub fn get(&self) -> Region {
        let data = unsafe { &self.datas.get().deref()[self.this] };
        Region::from_rects2(data)
    }
}
