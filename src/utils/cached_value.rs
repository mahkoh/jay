use crate::theme::Color;
use crate::theme::ContainerBordersSetting;
use crate::utils::clonecell::CloneCell;
use linearize::Linearize;
use linearize::StaticCopyMap;
use linearize::StaticMap;
use std::cell::Cell;
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::LazyLock;

pub trait CachedDefault: Sized {
    #[expect(unused)]
    fn cached_default() -> Self;
}

#[expect(unused)]
pub trait CachedValue: CachedDefault {
    type Changed: Copy + Debug + Default;
    type Op;

    fn cached_set(&self, v: Self);
    fn cached_apply(&self, v: Self::Op);
    fn cached_update(&self, v: Self, handle_change: impl FnMut(Self::Op)) -> Self::Changed;
}

impl<V> CachedDefault for Cell<V>
where
    V: CachedDefault,
{
    fn cached_default() -> Self {
        Cell::new(V::cached_default())
    }
}

impl<V> CachedValue for Cell<V>
where
    V: CachedDefault + Copy + PartialEq,
{
    type Changed = bool;
    type Op = V;

    fn cached_set(&self, v: Self) {
        self.set(v.into_inner())
    }

    fn cached_apply(&self, v: Self::Op) {
        self.set(v)
    }

    fn cached_update(&self, v: Self, mut handle_change: impl FnMut(Self::Op)) -> Self::Changed {
        let v = v.get();
        let changed = self.replace(v) != v;
        if changed {
            handle_change(v);
        }
        changed
    }
}

impl<V> CachedDefault for CloneCell<V>
where
    V: CachedDefault,
{
    fn cached_default() -> Self {
        CloneCell::new(V::cached_default())
    }
}

impl<T> CachedValue for CloneCell<T>
where
    T: CachedDefault + Clone + PartialEq,
{
    type Changed = bool;
    type Op = T;

    fn cached_set(&self, v: Self) {
        self.set(v.into_inner());
    }

    fn cached_apply(&self, v: Self::Op) {
        self.set(v);
    }

    fn cached_update(&self, v: Self, mut handle_change: impl FnMut(Self::Op)) -> Self::Changed {
        let v = v.into_inner();
        let old = self.set(v.clone());
        let changed = old != v;
        if changed {
            handle_change(v);
        }
        changed
    }
}

impl<K, V> CachedDefault for StaticMap<K, V>
where
    K: Linearize,
    V: CachedDefault,
{
    fn cached_default() -> Self {
        StaticMap::from_fn(|_| V::cached_default())
    }
}

impl<K, V> CachedValue for StaticMap<K, V>
where
    K: Linearize + Copy + Debug,
    V: CachedValue,
{
    type Changed = StaticCopyMap<K, V::Changed>;
    type Op = (K, V::Op);

    fn cached_set(&self, v: Self) {
        for (k, v) in v {
            V::cached_set(&self[k], v);
        }
    }

    fn cached_apply(&self, (k, v): Self::Op) {
        V::cached_apply(&self[k], v);
    }

    fn cached_update(&self, v: Self, mut handle_change: impl FnMut(Self::Op)) -> Self::Changed {
        let mut changed = StaticCopyMap::default();
        for (k, v) in v {
            changed[k] = V::cached_update(&self[k], v, |c| {
                handle_change((k, c));
            });
        }
        changed
    }
}

macro_rules! default {
    ($($ty:ty,)*) => {
        $(
            impl CachedDefault for $ty {
                fn cached_default() -> Self {
                    Default::default()
                }
            }
        )*
    };
}

default! {
    i32,
    bool,
    ContainerBordersSetting,
}

impl CachedDefault for Color {
    fn cached_default() -> Self {
        Color::TRANSPARENT
    }
}

impl CachedDefault for Arc<String> {
    fn cached_default() -> Self {
        static EMPTY_STRING: LazyLock<Arc<String>> = LazyLock::new(Default::default);
        EMPTY_STRING.clone()
    }
}
