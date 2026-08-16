use crate::utils::box_cache::BoxReset;
use crate::utils::box_cache::CachedBox;
use crate::utils::fuse::fuse_inode::FuseInode;
use crate::utils::fuse::fuse_inode::FuseInodePropsPti;
use crate::utils::fuse::fuse_inode::FuseInodeTy;
use crate::utils::fuse::fuse_inode::FuseInodeWithKey;
use crate::utils::fuse::fuse_inode_cache::InodeCache;
use crate::utils::fuse::fuse_inode_cache::fuse_inode_cache_types::ParentKeyRef;
use crate::utils::fuse::fuse_mgr::FuseIno;
use crate::utils::fuse::fuse_sys::fuse_dirent;
use crate::utils::fuse::fuse_sys::fuse_direntplus;
use crate::utils::fuse::fuse_sys::fuse_direntplus_size;
use crate::utils::fuse::fuse_sys::fuse_entry_out;
use jay_proc::Reset;
use std::ops::Range;
use std::rc::Rc;
use std::rc::Weak;
use uapi::c;
use uapi::pod_zeroed;

mod fuse_snapshot;

pub const FUSE_NO_TIMEOUT: u64 = u64::MAX;
pub const FUSE_SHORT_TIMEOUT: u64 = 100_000_000;

pub struct FuseDirent {
    pub inode: Rc<dyn FuseInode>,
    pub key: u64,
    pub static_name: Option<&'static str>,
    pub timeout_ns: u64,
}

pub(super) struct FuseOpenDir {
    pub(super) ino: FuseIno,
    pub(super) inode: Weak<dyn FuseInode>,
    pub(super) key: u64,
    pub(super) depth: u64,
    pub(super) dirents: CachedBox<FuseDirents, BoxReset>,
    pub(super) have_dirents: bool,
}

#[derive(Default, Reset)]
pub struct FuseDirents {
    dirents: Vec<Dirent>,
    names: String,
}

struct Dirent {
    inode: Option<DirentInode>,
    name: DirentName,
}

enum DirentName {
    Static(&'static str),
    Dynamic(Range<usize>),
}

struct DirentInode {
    inode: Weak<dyn FuseInode>,
    props: FuseInodePropsPti,
    key: u64,
    timeout_ns: u64,
}

pub enum FuseDirentName<'a> {
    Static(&'static str),
    Dynamic(&'a str),
}

impl<'a> From<&'a str> for FuseDirentName<'a> {
    fn from(value: &'a str) -> Self {
        Self::Dynamic(value)
    }
}

impl DirentName {
    fn get<'a>(&self, ents: &'a FuseDirents) -> &'a str {
        match self {
            DirentName::Static(n) => n,
            DirentName::Dynamic(r) => &ents.names[r.clone()],
        }
    }

    fn static_name(&self) -> Option<&'static str> {
        match self {
            DirentName::Static(n) => Some(*n),
            DirentName::Dynamic(_) => None,
        }
    }
}

impl FuseDirents {
    #[expect(unused)]
    pub fn add<I>(&mut self, timeout_ns: u64, inode: &Rc<I>, key: u64, name: FuseDirentName<'_>)
    where
        I: FuseInode + 'static,
    {
        let inode = DirentInode {
            props: inode.props_pti(key),
            inode: inode.downgrade(),
            key,
            timeout_ns,
        };
        self.add_(Some(inode), name);
    }

    pub fn add_dyn(
        &mut self,
        timeout_ns: u64,
        FuseInodeWithKey { inode, key }: FuseInodeWithKey,
        name: FuseDirentName<'_>,
    ) {
        let inode = DirentInode {
            props: inode.props_pti(key),
            inode: Rc::downgrade(&inode),
            key,
            timeout_ns,
        };
        self.add_(Some(inode), name);
    }

    fn add_(&mut self, inode: Option<DirentInode>, name: FuseDirentName<'_>) {
        let name = match name {
            FuseDirentName::Static(n) => DirentName::Static(n),
            FuseDirentName::Dynamic(n) => {
                let lo = self.names.len();
                self.names.push_str(n);
                let hi = self.names.len();
                DirentName::Dynamic(lo..hi)
            }
        };
        self.dirents.push(Dirent { inode, name });
    }
}

impl Dirent {
    fn dirent(&self, name: &str, idx: usize, ino: Option<FuseIno>) -> fuse_dirent {
        let mut ty = c::DT_DIR;
        if let Some(inode) = &self.inode {
            ty = match inode.props.ty {
                FuseInodeTy::Regular => c::DT_REG,
                FuseInodeTy::Directory => c::DT_DIR,
                FuseInodeTy::Symlink => c::DT_LNK,
            };
        };
        fuse_dirent {
            ino: ino.map(|v| v.0.get()).unwrap_or(0),
            off: (idx + 1) as u64,
            namelen: name.len() as _,
            type_: ty as _,
            name: [],
        }
    }

    fn direntplus(&self, name: &str, idx: usize, ino: Option<FuseIno>) -> fuse_direntplus {
        let mut attr = pod_zeroed();
        let mut entry_valid = 0;
        let mut entry_valid_nsec = 0;
        if let Some(inode) = &self.inode {
            attr = inode.props.props.attr(ino);
            entry_valid = inode.timeout_ns / 1_000_000_000;
            entry_valid_nsec = (inode.timeout_ns % 1_000_000_000) as u32;
        }
        let dirent = self.dirent(name, idx, ino);
        fuse_direntplus {
            entry_out: fuse_entry_out {
                nodeid: dirent.ino,
                attr,
                attr_valid: u64::MAX,
                entry_valid,
                entry_valid_nsec,
                ..pod_zeroed()
            },
            dirent,
        }
    }
}

impl FuseOpenDir {
    pub(super) fn ensure(&mut self) {
        if self.have_dirents {
            return;
        }
        self.have_dirents = true;
        self.dirents.add_(None, FuseDirentName::Static("."));
        self.dirents.add_(None, FuseDirentName::Static(".."));
        if let Some(v) = self.inode.upgrade() {
            v.getdents(self.key, &mut self.dirents);
        }
    }

    pub(super) fn encode_plus(
        &mut self,
        inodes: &InodeCache,
        offset: usize,
        size: usize,
        dst: &mut Vec<u8>,
    ) {
        self.ensure();
        let dirents = &self.dirents;
        for (idx, ent) in dirents.dirents.iter().enumerate().skip(offset) {
            let name = ent.name.get(dirents);
            let len = dst.len() + fuse_direntplus_size(name.len());
            if len > size {
                break;
            }
            let mut ino = None;
            if let Some(dirent) = &ent.inode {
                let parent = ParentKeyRef::new(self.ino, ent.name.static_name(), name);
                ino = Some(
                    inodes
                        .lookup2(
                            Some(parent),
                            self.depth,
                            dirent.inode.as_ptr().addr(),
                            || dirent.inode.clone(),
                            dirent.key,
                            dirent.props,
                        )
                        .ino,
                );
            }
            let plus = ent.direntplus(name, idx, ino);
            dst.extend_from_slice(uapi::as_bytes(&plus));
            dst.extend_from_slice(name.as_bytes());
            dst.resize(len, 0);
        }
    }
}
