/*
dir globals {
    next_name: reg,
    registry: view (key = 0),
    removed: view (key = 0),
    outputs: view (key = 0),
    seats: view (key = 0),
    singletons: view (key = 0),
    exposed: reg,
}

dir dfs_global (abstract, global) {
    global_name: reg,
    global_interface: reg,
    global_version: reg,
}

dir generic_global {
    global_name: view (no_timeout),
    global_interface: view (no_timeout),
    global_version: view (no_timeout),
}
 */
use crate::dfs::dfs_helpers::DfsGlobalLink;
use crate::globals::Global;
use crate::globals::GlobalName;
use crate::globals::Singleton;
use crate::globals::globals_dfs_g_fuse::generated::generic_global;
use crate::globals::globals_dfs_g_fuse::generated::globals;
use crate::state::State;
use crate::utils::fuse::fuse_globals::dfs_global;
use crate::utils::fuse::fuse_inode::FuseInodeExt;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::fuse::fuse_views::FuseLink;
use crate::utils::fuse::fuse_views::FuseReg;
use crate::utils::fuse::fuse_views::FuseRegView;
use crate::utils::fuse::fuse_views::IterDirDyn;
use crate::utils::fuse::fuse_views::IterDirDynView;
use crate::utils::fuse::fuse_views::IterDirKeyed;
use crate::utils::fuse::fuse_views::IterDirKeyedView;
use crate::utils::liveness::GetLiveness;
use crate::utils::static_rc::static_rc;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmt;
use crate::utils::type_view::TypeViewExt1;
use linearize::Linearize;
use linearize::LinearizeExt;
use smallvec::SmallVec;
use std::rc::Rc;
use std::str::FromStr;

pub type DfsGlobalsView = globals::View;

impl globals::Dir for State {
    type ViewRegistry = IterDirDyn<Registry>;
    type ViewRemoved = IterDirDyn<Removed>;
    type ViewOutputs = IterDirKeyed<Outputs>;
    type ViewSeats = IterDirKeyed<Seats>;
    type ViewSingletons = IterDirKeyed<Singletons>;

    fn read_next_name(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.globals.next_name.get().str_fmt(buf, ctx);
    }

    fn read_exposed(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        let mut v = SmallVec::<<Singleton as Linearize>::Storage<&str>>::new();
        for k in Singleton::variants() {
            if self.globals.exposed[k].get()
                && let Some(global) = self.globals.registry.get(&self.globals.singletons[k].name)
            {
                v.push(global.interface().name());
            }
        }
        v.str_fmt(buf, ctx);
    }
}

struct Registry;

impl IterDirDynView<State> for Registry {
    fn iter(t: &Rc<State>, _key: u64, mut f: impl FnMut(&str, FuseInodeWithKey)) {
        let mut buf = itoa::Buffer::new();
        for (name, global) in t.globals.registry.lock().iter() {
            f(buf.format(name.0), global.clone().debugfs(t));
        }
    }

    fn get(t: &Rc<State>, _key: u64, name: &str) -> Option<FuseInodeWithKey> {
        let name = u32::from_str(name).ok().map(GlobalName)?;
        let g = t.globals.registry.get(&name)?;
        Some(g.debugfs(t))
    }
}

struct Removed;

impl IterDirDynView<State> for Removed {
    fn iter(t: &Rc<State>, _key: u64, mut f: impl FnMut(&str, FuseInodeWithKey)) {
        let mut buf = itoa::Buffer::new();
        for (name, global) in t.globals.removed.lock().iter() {
            f(buf.format(name.0), global.clone().debugfs(t));
        }
    }

    fn get(t: &Rc<State>, _key: u64, name: &str) -> Option<FuseInodeWithKey> {
        let name = u32::from_str(name).ok().map(GlobalName)?;
        let g = t.globals.removed.get(&name)?;
        Some(g.debugfs(t))
    }
}

macro_rules! name_link {
    () => {
        fn get(_t: Rc<State>, _key: u64, name: &str) -> Option<(Rc<Self::Value>, u64)> {
            let name = u32::from_str(name).ok().map(GlobalName)?;
            Some((static_rc().clone(), name.raw() as _))
        }
    };
}

struct Outputs;

impl IterDirKeyedView<State> for Outputs {
    type Value = ();
    type View = FuseLink<DfsGlobalLink>;

    fn iter(t: Rc<State>, _key: u64, mut f: impl FnMut(&str, &Rc<Self::Value>, u64)) {
        let mut buf = itoa::Buffer::new();
        for o in t.globals.outputs.lock().keys() {
            f(buf.format(o.0), static_rc(), o.0 as u64);
        }
    }

    name_link!();
}

struct Seats;

impl IterDirKeyedView<State> for Seats {
    type Value = ();
    type View = FuseLink<DfsGlobalLink>;

    fn iter(t: Rc<State>, _key: u64, mut f: impl FnMut(&str, &Rc<Self::Value>, u64)) {
        let mut buf = itoa::Buffer::new();
        for o in t.globals.seats.lock().keys() {
            f(buf.format(o.0), static_rc(), o.0 as u64);
        }
    }

    name_link!();
}

struct Singletons;

impl IterDirKeyedView<State> for Singletons {
    type Value = ();
    type View = FuseLink<DfsGlobalLink>;

    fn iter(t: Rc<State>, _key: u64, mut f: impl FnMut(&str, &Rc<Self::Value>, u64)) {
        for (singleton, info) in t.globals.singletons.iter() {
            f(
                singleton.interface().name(),
                static_rc(),
                info.name.raw() as _,
            );
        }
    }

    fn get(t: Rc<State>, _key: u64, name: &str) -> Option<(Rc<Self::Value>, u64)> {
        for (singleton, info) in t.globals.singletons.iter() {
            if singleton.interface().name() == name {
                return Some((static_rc().clone(), info.name.raw() as _));
            }
        }
        None
    }
}

impl<T> dfs_global::Dir for T
where
    T: Global + GetLiveness + 'static,
{
    fn read_global_name(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.name().0.str_fmt(buf, ctx);
    }

    fn read_global_interface(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.interface().name().str_fmt(buf, ctx);
    }

    fn read_global_version(&self, buf: &mut String, ctx: &StrCtx<'_>) {
        self.version().str_fmt(buf, ctx);
    }
}

impl State {
    pub fn dfs_generic_global(self: &Rc<Self>, name: GlobalName) -> FuseInodeWithKey {
        self.tv_wrap_rc_ref_clone::<generic_global::View>()
            .with_key(name.0 as u64)
    }
}

impl generic_global::Dir for State {
    type ViewGlobalName = FuseReg<GenericGlobalId>;
    type ViewGlobalInterface = FuseReg<GenericGlobalInterface>;
    type ViewGlobalVersion = FuseReg<GenericGlobalVersion>;

    fn keyof_global_name(&self, key: u64) -> u64 {
        key
    }

    fn keyof_global_interface(&self, key: u64) -> u64 {
        key
    }

    fn keyof_global_version(&self, key: u64) -> u64 {
        key
    }
}

struct GenericGlobalId;

impl FuseRegView<State> for GenericGlobalId {
    fn read(_t: &State, key: u64, buf: &mut String, ctx: &StrCtx<'_>) {
        key.str_fmt(buf, ctx);
    }
}

struct GenericGlobalInterface;

impl FuseRegView<State> for GenericGlobalInterface {
    fn read(t: &State, key: u64, buf: &mut String, ctx: &StrCtx<'_>) {
        let name = GlobalName(key as u32);
        let globals = &t.globals;
        if let Some(g) = globals
            .registry
            .get(&name)
            .or_else(|| globals.removed.get(&name))
        {
            g.interface().name().str_fmt(buf, ctx);
        }
    }
}

struct GenericGlobalVersion;

impl FuseRegView<State> for GenericGlobalVersion {
    fn read(t: &State, key: u64, buf: &mut String, ctx: &StrCtx<'_>) {
        let name = GlobalName(key as u32);
        let globals = &t.globals;
        if let Some(g) = globals
            .registry
            .get(&name)
            .or_else(|| globals.removed.get(&name))
        {
            g.version().str_fmt(buf, ctx);
        }
    }
}

// FUSE GENERATED START
include!(concat!(
    env!("OUT_DIR"),
    "/fuse/m_153af2ca561922e04816e5df06e9def29c5a842a49bd5304ee1e0d88ae221eba.rs",
));
// FUSE GENERATED STOP
