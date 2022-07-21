use {
    crate::rect::{Rect, Region},
    algorithms::rect::{
        region::{extents, rects_to_bands, subtract, union},
        RectRaw,
    },
    once_cell::unsync::Lazy,
    smallvec::SmallVec,
    std::{mem, ops::Deref, rc::Rc},
};

#[thread_local]
static EMPTY: Lazy<Rc<Region>> = Lazy::new(|| {
    Rc::new(Region {
        rects: Default::default(),
        extents: Default::default(),
    })
});

impl Region {
    pub fn new(rect: Rect) -> Rc<Self> {
        let mut rects = SmallVec::new();
        rects.push(rect.raw);
        Rc::new(Self {
            rects,
            extents: rect,
        })
    }

    pub fn empty() -> Rc<Self> {
        EMPTY.clone()
    }

    pub fn from_rects(rects: &[Rect]) -> Rc<Self> {
        if rects.is_empty() {
            return Self::empty();
        }
        if rects.len() == 1 {
            return Self::new(rects[0]);
        }
        let rects = rects_to_bands(unsafe { mem::transmute(rects) });
        Rc::new(Self {
            extents: Rect {
                raw: extents(&rects),
            },
            rects,
        })
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

    #[allow(dead_code)]
    pub fn extents(&self) -> Rect {
        self.extents
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

impl Deref for Region {
    type Target = [Rect];

    fn deref(&self) -> &Self::Target {
        unsafe { mem::transmute::<&[RectRaw], _>(&self.rects) }
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

    #[allow(dead_code)]
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
