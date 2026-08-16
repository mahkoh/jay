use crate::utils::box_cache::BoxCache;
use crate::utils::box_cache::BoxReset;
use crate::utils::box_cache::CachedBox;
use crate::utils::fuse::fuse_inode::FuseInode;
use crate::utils::str_fmt::StrCtx;
use jay_algorithms::oserror::ESTALE;
use jay_algorithms::oserror::OsError;
use std::rc::Rc;
use std::rc::Weak;

pub(super) struct FuseOpenReg {
    pub(super) inode: Weak<dyn FuseInode>,
    pub(super) key: u64,
    pub(super) ctx: StrCtx<'static>,
    pub(super) contents: Option<Rc<CachedBox<String, BoxReset>>>,
}

impl FuseOpenReg {
    pub fn read(
        &mut self,
        cache: &Rc<BoxCache<String, BoxReset>>,
    ) -> Result<Rc<CachedBox<String, BoxReset>>, OsError> {
        if let Some(v) = &self.contents {
            return Ok(v.clone());
        }
        let Some(inode) = self.inode.upgrade() else {
            return ESTALE();
        };
        let mut contents = cache.get();
        inode.read(self.key, &mut contents, &self.ctx);
        let contents = Rc::from(contents);
        self.contents = Some(contents.clone());
        Ok(contents)
    }
}
