#![allow(dead_code)]

use {
    crate::{
        utils::{
            copyhashmap::CopyHashMap,
            errorfmt::ErrorFmt,
            linkedlist::{LinkedList, LinkedNode},
            oserror::OsError,
        },
        video::drm::{
            sys::{
                syncobj_create, syncobj_destroy, syncobj_eventfd, syncobj_fd_to_handle,
                syncobj_handle_to_fd, syncobj_query, DRM_SYNCOBJ_QUERY_FLAGS_LAST_SUBMITTED,
                DRM_SYNCOBJ_WAIT_FLAGS_WAIT_AVAILABLE,
            },
            DrmError,
        },
    },
    std::{
        rc::Rc,
        sync::atomic::{AtomicU64, Ordering::Relaxed},
    },
    uapi::{c, OwnedFd},
};

static SYNCOBJ_ID: AtomicU64 = AtomicU64::new(0);

pub struct SyncObj {
    id: u64,
    fd: Rc<OwnedFd>,
    importers: LinkedList<Rc<Handles>>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct SyncObjPoint(pub u64);

impl SyncObj {
    pub fn new(fd: &Rc<OwnedFd>) -> Self {
        Self {
            id: SYNCOBJ_ID.fetch_add(1, Relaxed),
            fd: fd.clone(),
            importers: Default::default(),
        }
    }

    pub fn fd(&self) -> &Rc<OwnedFd> {
        &self.fd
    }
}

impl Drop for SyncObj {
    fn drop(&mut self) {
        let mut links = vec![];
        for importer in self.importers.iter() {
            if let Some(handle) = importer.handles.remove(&self.id) {
                destroy(&importer.drm, handle);
            }
            if let Some(link) = importer.links.remove(&self.id) {
                links.push(link);
            }
        }
    }
}

struct Handles {
    drm: Rc<OwnedFd>,
    handles: CopyHashMap<u64, u32>,
    links: CopyHashMap<u64, LinkedNode<Rc<Handles>>>,
}

pub struct SyncObjCtx {
    inner: Rc<Handles>,
}

impl SyncObjCtx {
    pub fn new(drm: &Rc<OwnedFd>) -> Self {
        Self {
            inner: Rc::new(Handles {
                drm: drm.clone(),
                handles: Default::default(),
                links: Default::default(),
            }),
        }
    }

    fn get_handle(&self, syncobj: &SyncObj) -> Result<u32, DrmError> {
        if let Some(handle) = self.inner.handles.get(&syncobj.id) {
            return Ok(handle);
        }
        let handle = syncobj_fd_to_handle(self.inner.drm.raw(), syncobj.fd.raw(), 0)
            .map_err(DrmError::ImportSyncObj)?;
        let link = syncobj.importers.add_last(self.inner.clone());
        self.inner.handles.set(syncobj.id, handle);
        self.inner.links.set(syncobj.id, link);
        Ok(handle)
    }

    pub fn create_sync_obj(&self) -> Result<SyncObj, DrmError> {
        let handle = syncobj_create(self.inner.drm.raw(), 0).map_err(DrmError::CreateSyncObj)?;
        let fd = syncobj_handle_to_fd(self.inner.drm.raw(), handle, 0);
        if fd.is_err() {
            destroy(&self.inner.drm, handle);
        }
        let fd = fd.map_err(DrmError::ExportSyncObj).map(Rc::new)?;
        let syncobj = SyncObj::new(&fd);
        let link = syncobj.importers.add_last(self.inner.clone());
        self.inner.handles.set(syncobj.id, handle);
        self.inner.links.set(syncobj.id, link);
        Ok(syncobj)
    }

    pub fn wait_for_point(
        &self,
        eventfd: &OwnedFd,
        syncobj: &SyncObj,
        point: SyncObjPoint,
        signaled: bool,
    ) -> Result<(), DrmError> {
        let handle = self.get_handle(syncobj)?;
        let flags = match signaled {
            true => 0,
            false => DRM_SYNCOBJ_WAIT_FLAGS_WAIT_AVAILABLE,
        };
        syncobj_eventfd(self.inner.drm.raw(), eventfd.raw(), handle, point.0, flags)
            .map_err(DrmError::RegisterEventfd)
    }

    pub fn get_last_point(
        &self,
        syncobj: &SyncObj,
        signaled: bool,
    ) -> Result<SyncObjPoint, DrmError> {
        let handle = self.get_handle(syncobj)?;
        let flags = match signaled {
            true => 0,
            false => DRM_SYNCOBJ_QUERY_FLAGS_LAST_SUBMITTED,
        };
        let res = syncobj_query(self.inner.drm.raw(), handle, flags);
        match res {
            Ok(p) => Ok(SyncObjPoint(p)),
            Err(e) => Err(DrmError::LastPoint(e)),
        }
    }

    pub fn is_ready(
        &self,
        syncobj: &SyncObj,
        point: SyncObjPoint,
        signaled: bool,
    ) -> Result<bool, DrmError> {
        Ok(self.get_last_point(syncobj, signaled)? >= point)
    }

    pub fn supports_async_wait(&self) -> bool {
        self.supports_async_wait_().is_ok()
    }

    fn supports_async_wait_(&self) -> Result<(), DrmError> {
        let syncobj = self.create_sync_obj()?;
        let eventfd = uapi::eventfd(0, c::EFD_CLOEXEC)
            .map_err(OsError::from)
            .map_err(DrmError::EventFd)?;
        self.wait_for_point(&eventfd, &syncobj, SyncObjPoint(1), true)?;
        Ok(())
    }
}

impl Drop for SyncObjCtx {
    fn drop(&mut self) {
        self.inner.links.clear();
        let mut map = self.inner.handles.lock();
        for (_, handle) in map.drain() {
            destroy(&self.inner.drm, handle);
        }
    }
}

fn destroy(drm: &OwnedFd, handle: u32) {
    if let Err(e) = syncobj_destroy(drm.raw(), handle) {
        log::error!("Could not destroy syncobj: {}", ErrorFmt(e));
    }
}
