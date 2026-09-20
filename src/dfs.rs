use crate::state::State;
use crate::utils::copyhashmap::CopyHashMap;
use crate::utils::errorfmt::ErrorFmt;
use crate::utils::fuse::fuse_error::FuseError;
use crate::utils::fuse::fuse_mount::FuseMount;
use crate::utils::fuse::fuse_mount::FuseMountOwner;
use crate::utils::hash_map_ext::HashMapExt;
use crate::utils::numcell::NumCell;
use crate::utils::unique_process_id::unique_process_id;
use run_on_drop::on_drop;
use std::cell::Cell;
use std::cell::OnceCell;
use std::cell::RefCell;
use std::rc::Rc;

pub mod dfs_helpers;

pub trait DfsMounter {
    fn success(self: Rc<Self>, path: &str);
    fn failure(self: Rc<Self>);
}

#[derive(Default)]
pub struct DfsManager {
    inner: Rc<Inner>,
}

#[derive(Default)]
struct Inner {
    next_dir_id: NumCell<u64>,
    next_request_id: NumCell<u64>,
    mount: RefCell<Option<FuseMount>>,
    path: OnceCell<Rc<String>>,
    mounted: Cell<bool>,
    mounters: CopyHashMap<u64, Rc<RequestInner>>,
}

impl DfsManager {
    pub fn clear(&self) {
        self.inner.mounters.clear();
        self.inner.mount.take();
    }
}

pub struct PendingDfsMount {
    id: u64,
    request: Rc<RequestInner>,
    inner: Rc<Inner>,
}

struct RequestInner {
    mounter: Cell<Option<Rc<dyn DfsMounter>>>,
}

impl RequestInner {
    fn success(&self, path: &str) {
        if let Some(m) = self.mounter.take() {
            m.success(path);
        }
    }

    fn failure(&self) {
        if let Some(m) = self.mounter.take() {
            m.failure();
        }
    }
}

impl State {
    pub fn mount_debugfs(self: &Rc<Self>, mounter: Rc<dyn DfsMounter>) -> PendingDfsMount {
        let inner = &self.debugfs.inner;
        let ri = Rc::new(RequestInner {
            mounter: Cell::new(Some(mounter)),
        });
        let pending = PendingDfsMount {
            id: inner.next_request_id.fetch_add(1),
            request: ri.clone(),
            inner: inner.clone(),
        };
        let fail = on_drop({
            let ri = ri.clone();
            || self.run_toplevel.schedule(move || ri.failure())
        });
        let Some(jay_dir) = self.jay_runtime_dir.get() else {
            log::error!("Jay runtime dir is not set");
            return pending;
        };
        let path = inner
            .path
            .get_or_init(|| Rc::new(format!("{jay_dir}/debugfs")));
        if !inner.mounted.get() {
            let mount = &mut *inner.mount.borrow_mut();
            if mount.is_none() {
                let id = inner.next_dir_id.fetch_add(1);
                let unique = unique_process_id();
                let dfs_dir = format!("{jay_dir}/.debugfs");
                if let Err(e) = std::fs::create_dir_all(&dfs_dir) {
                    log::error!("Could not create {dfs_dir}: {}", ErrorFmt(e));
                    return pending;
                }
                for file in std::fs::read_dir(&dfs_dir).ok().into_iter().flatten() {
                    let Ok(file) = file else {
                        continue;
                    };
                    let _ = std::fs::remove_dir(file.path());
                }
                let local_mount_dir = format!(".debugfs/{unique}.{id}");
                let mount_dir = format!("{jay_dir}/{local_mount_dir}");
                if let Err(e) = std::fs::create_dir_all(&mount_dir) {
                    log::error!("Could not create {mount_dir}: {}", ErrorFmt(e));
                    return pending;
                }
                let symlink = format!("{jay_dir}/debugfs.{unique}.{id}");
                if let Err(e) = uapi::symlink(&*local_mount_dir, &*symlink) {
                    log::error!("Could not symlink {symlink}: {}", ErrorFmt(e));
                    return pending;
                }
                if let Err(e) = uapi::rename(&*symlink, &***path) {
                    log::error!("Could not rename {symlink} to {path}: {}", ErrorFmt(e));
                    return pending;
                }
                let mnt = self.fuse_mount(inner.clone(), self.debugfs(), &mount_dir);
                *mount = Some(mnt);
            }
        }
        fail.forget();
        if inner.mounted.get() {
            let path = path.clone();
            self.run_toplevel.schedule(move || ri.success(&path));
        } else {
            inner.mounters.set(pending.id, ri);
        }
        pending
    }

    pub fn unmount_debugfs(&self) {
        let inner = &self.debugfs.inner;
        inner.mounted.set(false);
        inner.mount.take();
        for mounter in inner.mounters.lock().drain_values() {
            self.run_toplevel.schedule(move || mounter.failure());
        }
    }
}

impl FuseMountOwner for Inner {
    fn success(&self) {
        self.mounted.set(true);
        let path = self.path.get().unwrap();
        log::info!("Debugfs was mounted at {path}");
        let mut mounters = self.mounters.clear();
        for mounter in mounters.drain_values() {
            mounter.success(path);
        }
    }

    fn failed(self: Rc<Self>, error: FuseError) {
        if let FuseError::Aborted = error {
            log::info!("Debugfs was unmounted");
        } else {
            log::error!("Debugfs mount failed: {}", ErrorFmt(error));
        }
        self.mounted.set(false);
        self.mount.take();
        let mut mounters = self.mounters.clear();
        for mounter in mounters.drain_values() {
            mounter.failure();
        }
    }
}

impl Drop for PendingDfsMount {
    fn drop(&mut self) {
        self.inner.mounters.remove(&self.id);
        self.request.mounter.take();
    }
}
