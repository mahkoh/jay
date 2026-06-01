use {
    crate::{
        ifs::wl_surface::commit_timeline::Commit,
        utils::{box_ext::BoxExt, stack::Stack},
    },
    std::{
        mem::{ManuallyDrop, MaybeUninit},
        ops::Deref,
        rc::Rc,
    },
};

#[derive(Default)]
pub struct CommitCache {
    commits: Stack<Box<MaybeUninit<Commit>>>,
}

pub(super) struct CachedCommit {
    cache: Rc<CommitCache>,
    commit: ManuallyDrop<Box<Commit>>,
}

impl Deref for CachedCommit {
    type Target = Commit;

    fn deref(&self) -> &Self::Target {
        &self.commit
    }
}

impl CommitCache {
    pub(super) fn get(self: &Rc<Self>, commit: Commit) -> CachedCommit {
        let c = self
            .commits
            .pop()
            .unwrap_or_else(|| Box::new(MaybeUninit::uninit()));
        let c = Box::write(c, commit);
        CachedCommit {
            cache: self.clone(),
            commit: ManuallyDrop::new(c),
        }
    }
}

impl Drop for CachedCommit {
    fn drop(&mut self) {
        let commit = unsafe { ManuallyDrop::take(&mut self.commit) };
        let commit = Box::into_uninit(commit);
        self.cache.commits.push(commit);
    }
}
