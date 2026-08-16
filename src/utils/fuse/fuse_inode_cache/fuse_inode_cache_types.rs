use crate::utils::fuse::fuse_inode_cache::CachedInode;
use crate::utils::fuse::fuse_mgr::FuseIno;
use hashbrown::Equivalent;
use std::any::TypeId;
use std::hash::Hash;
use std::hash::Hasher;
use std::rc::Rc;

impl CachedInode {
    pub(super) fn key(&self) -> FuseInodeKey {
        FuseInodeKey {
            parent: self.parent.clone(),
            type_id: self.props.type_id,
            addr: self.inode.as_ptr().addr(),
            key: self.props.key,
            depth: self.props.depth,
        }
    }
}

#[derive(Copy, Clone, Debug)]
enum DentryNameRef<'a> {
    Static(&'static str),
    Dynamic(&'a str),
}

#[derive(Clone, Debug)]
enum DentryName {
    Static(&'static str),
    Dynamic(Rc<str>),
}

impl DentryName {
    fn as_str(&self) -> &str {
        match self {
            DentryName::Static(v) => v,
            DentryName::Dynamic(v) => v,
        }
    }
}

impl DentryNameRef<'_> {
    fn as_str(&self) -> &str {
        match self {
            DentryNameRef::Static(v) => v,
            DentryNameRef::Dynamic(v) => v,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub(in super::super) struct ParentKeyRef<'a> {
    ino: FuseIno,
    dentry_name: DentryNameRef<'a>,
}

impl<'a> ParentKeyRef<'a> {
    pub fn new(ino: FuseIno, static_name: Option<&'static str>, name: &'a str) -> Self {
        Self {
            ino,
            dentry_name: match static_name {
                None => DentryNameRef::Dynamic(name),
                Some(name) => DentryNameRef::Static(name),
            },
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct FuseInodeKeyRef<'a> {
    pub(super) parent: Option<ParentKeyRef<'a>>,
    pub(super) type_id: TypeId,
    pub(super) addr: usize,
    pub(super) key: u64,
    pub(super) depth: u64,
}

impl DentryNameRef<'_> {
    fn to_owned(&self) -> DentryName {
        match self {
            DentryNameRef::Static(v) => DentryName::Static(v),
            DentryNameRef::Dynamic(v) => DentryName::Dynamic((*v).into()),
        }
    }
}

impl ParentKeyRef<'_> {
    fn to_owned(&self) -> ParentKey {
        ParentKey {
            ino: self.ino,
            dentry_name: self.dentry_name.to_owned(),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct ParentKey {
    ino: FuseIno,
    dentry_name: DentryName,
}

#[derive(Clone, Debug)]
pub(super) struct FuseInodeKey {
    pub(super) parent: Option<ParentKey>,
    type_id: TypeId,
    addr: usize,
    key: u64,
    depth: u64,
}

#[derive(PartialEq, Hash)]
struct FuseInodeKeyParentHash<'a> {
    ino: FuseIno,
    dentry_name: &'a str,
}

#[derive(PartialEq, Hash)]
struct FuseInodeKeyHash<'a> {
    parent: Option<FuseInodeKeyParentHash<'a>>,
    type_id: TypeId,
    addr: usize,
    key: u64,
    depth: u64,
}

macro_rules! impl_to_hash {
    ($slf:ident) => {
        FuseInodeKeyHash {
            parent: $slf.parent.as_ref().map(|p| FuseInodeKeyParentHash {
                ino: p.ino,
                dentry_name: p.dentry_name.as_str(),
            }),
            type_id: $slf.type_id,
            addr: $slf.addr,
            key: $slf.key,
            depth: $slf.depth,
        }
    };
}

impl<'a> FuseInodeKeyRef<'a> {
    fn to_hash(&self) -> FuseInodeKeyHash<'_> {
        impl_to_hash!(self)
    }
}

impl FuseInodeKey {
    fn to_hash(&self) -> FuseInodeKeyHash<'_> {
        impl_to_hash!(self)
    }
}

impl FuseInodeKeyRef<'_> {
    pub(super) fn to_owned(&self) -> FuseInodeKey {
        FuseInodeKey {
            parent: self.parent.map(|p| p.to_owned()),
            type_id: self.type_id,
            addr: self.addr,
            key: self.key,
            depth: self.depth,
        }
    }
}

impl Eq for FuseInodeKey {}

impl PartialEq for FuseInodeKey {
    fn eq(&self, other: &Self) -> bool {
        self.to_hash() == other.to_hash()
    }
}

impl Equivalent<FuseInodeKey> for FuseInodeKeyRef<'_> {
    fn equivalent(&self, other: &FuseInodeKey) -> bool {
        self.to_hash() == other.to_hash()
    }
}

macro_rules! impl_hash {
    ($l:ty) => {
        impl Hash for $l {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.to_hash().hash(state);
            }
        }
    };
}

impl_hash!(FuseInodeKey);
impl_hash!(FuseInodeKeyRef<'_>);
