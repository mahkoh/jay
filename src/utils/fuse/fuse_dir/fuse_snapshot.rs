use crate::utils::box_cache::BoxCache;
use crate::utils::box_cache::BoxReset;
use crate::utils::fuse::fuse_dir::FUSE_NO_TIMEOUT;
use crate::utils::fuse::fuse_dir::FuseDirents;
use crate::utils::fuse::fuse_error::FuseError;
use crate::utils::fuse::fuse_inode::FuseInode;
use crate::utils::fuse::fuse_inode::FuseInodeTy;
use crate::utils::fx_hash::FHashMap;
use crate::utils::numcell::NumCell;
use crate::utils::seekable_fd::SeekableFd;
use crate::utils::str_fmt::StrCtx;
use crate::utils::str_fmt::StrFmtFmt;
use hashbrown::hash_map::Entry;
use isnt::std_1::primitive::IsntSliceExt;
use jay_algorithms::jar::JarWriter;
use jay_algorithms::oserror::OsErrorExt2;
use jay_proc::Reset;
use std::any::TypeId;
use std::io;
use std::io::BufWriter;
use std::rc::Rc;
use uapi::OwnedFd;
use uapi::c;

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
struct RegKey {
    type_id: TypeId,
    addr: usize,
    key: u64,
}

struct RegValue {
    _inode: Rc<dyn FuseInode>,
    unique: u64,
}

#[derive(Default, Reset)]
struct DirentsWithOffset {
    dirents: FuseDirents,
    offset: usize,
}

impl dyn FuseInode {
    #[expect(unused)]
    pub fn snapshot(
        self: &Rc<Self>,
        key: u64,
        root: &str,
        json: bool,
    ) -> Result<Rc<OwnedFd>, FuseError> {
        if root.bytes().any(|b| b == 0) {
            return Err(FuseError::InvalidRootName);
        }
        let fd = uapi::memfd_create("jay-debugfs", c::MFD_CLOEXEC)
            .map(Rc::new)
            .map_os_err(FuseError::CreateMemfd)?;
        self.snapshot_(key, root, json, &fd)
            .map_err(FuseError::WriteMemfd)?;
        Ok(fd)
    }

    fn snapshot_(
        self: &Rc<Self>,
        key: u64,
        root: &str,
        json: bool,
        fd: &Rc<OwnedFd>,
    ) -> io::Result<()> {
        let mut buf_writer = BufWriter::new(SeekableFd(fd.raw()));
        let mut jar_writer = JarWriter::new(&mut buf_writer)?;
        let dirents_cache: Rc<BoxCache<DirentsWithOffset, BoxReset>> = Default::default();
        let mut parents = vec![];
        {
            let mut dirents = dirents_cache.get();
            dirents
                .dirents
                .add_dyn(FUSE_NO_TIMEOUT, self.clone().with_key(key), root.into());
            parents.push(dirents);
        }
        let str_ctx = StrCtx {
            fmt: match json {
                true => StrFmtFmt::Jsonl,
                false => StrFmtFmt::Human,
            },
            ..StrCtx::default()
        };
        let mut buf = String::new();
        let next_unique = NumCell::new(1);
        let mut reg_cache: FHashMap<RegKey, RegValue> = Default::default();
        while let Some(mut dirents_box) = parents.pop() {
            let dirents = &mut *dirents_box;
            loop {
                if dirents.offset >= dirents.dirents.dirents.len() {
                    if parents.is_not_empty() {
                        jar_writer.add_dir_up()?;
                    }
                    break;
                }
                let ent = &dirents.dirents.dirents[dirents.offset];
                dirents.offset += 1;
                let Some(dirent) = &ent.inode else {
                    continue;
                };
                let Some(inode) = dirent.inode.upgrade() else {
                    continue;
                };
                let key = dirent.key;
                let name = ent.name.get(&dirents.dirents).as_bytes();
                match dirent.props.ty {
                    FuseInodeTy::Regular => {
                        let reg_key = RegKey {
                            type_id: dirent.props.type_id,
                            addr: dirent.inode.as_ptr().addr(),
                            key: dirent.key,
                        };
                        match reg_cache.entry(reg_key) {
                            Entry::Occupied(v) => {
                                jar_writer.add_hrd(name, v.get().unique)?;
                            }
                            Entry::Vacant(v) => {
                                let unique = next_unique.fetch_add(1);
                                buf.clear();
                                inode.read(key, &mut buf, &str_ctx);
                                jar_writer.add_reg(name, unique, buf.as_bytes())?;
                                v.insert(RegValue {
                                    _inode: inode,
                                    unique,
                                });
                            }
                        }
                    }
                    FuseInodeTy::Symlink => {
                        let depth = parents.len() as u64;
                        buf.clear();
                        inode.readlink(key, depth, &mut buf);
                        jar_writer.add_lnk(name, buf.as_bytes())?;
                    }
                    FuseInodeTy::Directory => {
                        const MAX_DEPTH: usize = 128;
                        if parents.len() >= MAX_DEPTH {
                            log::error!("Maximum depth exceeded");
                        } else {
                            jar_writer.add_dir(name)?;
                            parents.push(dirents_box);
                            let mut dirents = dirents_cache.get();
                            inode.getdents(key, &mut dirents.dirents);
                            parents.push(dirents);
                            break;
                        }
                    }
                }
            }
        }
        jar_writer.finish()?;
        buf_writer.into_inner()?;
        Ok(())
    }
}
