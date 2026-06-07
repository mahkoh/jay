use {
    crate::utils::{
        cell_ext::CellExt,
        clonecell::{CloneCell, UnsafeCellCloneSafe},
    },
    std::cell::Cell,
};

pub trait ObjWithId {
    type Id: Copy;

    fn id(&self) -> Self::Id;
}

impl<T> ObjWithId for Option<T>
where
    T: ObjWithId,
{
    type Id = Option<T::Id>;

    fn id(&self) -> Self::Id {
        self.as_ref().map(ObjWithId::id)
    }
}

impl<T> ObjWithId for &T
where
    T: ObjWithId,
{
    type Id = T::Id;

    fn id(&self) -> Self::Id {
        <T as ObjWithId>::id(self)
    }
}

pub struct ObjAndId<T>
where
    T: ObjWithId,
{
    obj: CloneCell<T>,
    id: Cell<T::Id>,
}

impl<T> Default for ObjAndId<T>
where
    T: ObjWithId + Default,
{
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> ObjAndId<T>
where
    T: ObjWithId,
{
    pub fn new(obj: T) -> Self {
        Self {
            id: Cell::new(obj.id()),
            obj: CloneCell::new(obj),
        }
    }

    #[inline]
    pub fn get(&self) -> T
    where
        T: UnsafeCellCloneSafe,
    {
        self.obj.get()
    }

    #[inline]
    pub fn id(&self) -> T::Id {
        self.id.get()
    }

    pub fn set(&self, obj: T) -> T {
        self.id.set(obj.id());
        self.obj.set(obj)
    }
}

impl<T> ObjAndId<Option<T>>
where
    T: ObjWithId,
{
    #[inline]
    pub fn is_some(&self) -> bool {
        self.id.is_some()
    }

    #[inline]
    pub fn is_none(&self) -> bool {
        self.id.is_none()
    }
}
