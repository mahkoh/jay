use {
    crate::{
        gfx_api::SyncFile,
        utils::{
            clonecell::{CloneCell, UnsafeCellCloneSafe},
            copyhashmap::CopyHashMap,
            errorfmt::ErrorFmt,
            hash_map_ext::HashMapExt,
            linkedlist::{LinkedList, LinkedNode},
            oserror::OsError,
        },
        video::drm::{
            DrmError,
            sys::{
                DRM_SYNCOBJ_CREATE_SIGNALED, DRM_SYNCOBJ_FD_TO_HANDLE_FLAGS_IMPORT_SYNC_FILE,
                DRM_SYNCOBJ_HANDLE_TO_FD_FLAGS_EXPORT_SYNC_FILE,
                DRM_SYNCOBJ_WAIT_FLAGS_WAIT_AVAILABLE, sync_ioc_merge, sync_obj_create,
                sync_obj_destroy, sync_obj_eventfd, sync_obj_fd_to_handle, sync_obj_handle_to_fd,
                sync_obj_signal, sync_obj_transfer,
            },
        },
    },
    std::{
        rc::Rc,
        sync::atomic::{AtomicU64, Ordering::Relaxed},
    },
    uapi::{OwnedFd, c},
};

static SYNCOBJ_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct SyncObjId(u64);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct SyncObjHandle(u32);

unsafe impl UnsafeCellCloneSafe for SyncObjHandle {}

pub struct SyncObj {
    id: SyncObjId,
    fd: Rc<OwnedFd>,
    importers: LinkedList<Rc<Handles>>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct SyncObjPoint(pub u64);

impl SyncObj {
    pub fn new(fd: &Rc<OwnedFd>) -> Self {
        Self {
            id: SyncObjId(SYNCOBJ_ID.fetch_add(1, Relaxed)),
            fd: fd.clone(),
            importers: Default::default(),
        }
    }

    #[cfg_attr(not(feature = "it"), expect(dead_code))]
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
    handles: CopyHashMap<SyncObjId, SyncObjHandle>,
    links: CopyHashMap<SyncObjId, LinkedNode<Rc<Handles>>>,
}

pub struct SyncObjCtx {
    inner: Rc<Handles>,
    dummy: CloneCell<Option<Rc<SyncObj>>>,
}

impl SyncObjCtx {
    pub fn new(drm: &Rc<OwnedFd>) -> Self {
        Self {
            inner: Rc::new(Handles {
                drm: drm.clone(),
                handles: Default::default(),
                links: Default::default(),
            }),
            dummy: Default::default(),
        }
    }

    fn get_handle(&self, sync_obj: &SyncObj) -> Result<SyncObjHandle, DrmError> {
        if let Some(handle) = self.inner.handles.get(&sync_obj.id) {
            return Ok(handle);
        }
        let handle = sync_obj_fd_to_handle(self.inner.drm.raw(), sync_obj.fd.raw(), 0, 0)
            .map_err(DrmError::ImportSyncObj)?;
        let handle = SyncObjHandle(handle);
        let link = sync_obj.importers.add_last(self.inner.clone());
        self.inner.handles.set(sync_obj.id, handle);
        self.inner.links.set(sync_obj.id, link);
        Ok(handle)
    }

    pub fn create_sync_obj(&self) -> Result<SyncObj, DrmError> {
        let handle = sync_obj_create(self.inner.drm.raw(), 0).map_err(DrmError::CreateSyncObj)?;
        let handle = SyncObjHandle(handle);
        let fd = sync_obj_handle_to_fd(self.inner.drm.raw(), handle.0, 0);
        if fd.is_err() {
            destroy(&self.inner.drm, handle);
        }
        let fd = fd.map_err(DrmError::ExportSyncObj).map(Rc::new)?;
        let sync_obj = SyncObj::new(&fd);
        let link = sync_obj.importers.add_last(self.inner.clone());
        self.inner.handles.set(sync_obj.id, handle);
        self.inner.links.set(sync_obj.id, link);
        Ok(sync_obj)
    }

    pub fn create_signaled_sync_file(&self) -> Result<SyncFile, DrmError> {
        let handle = sync_obj_create(self.inner.drm.raw(), DRM_SYNCOBJ_CREATE_SIGNALED)
            .map_err(DrmError::CreateSyncObj)?;
        let handle = SyncObjHandle(handle);
        let fd = sync_obj_handle_to_fd(
            self.inner.drm.raw(),
            handle.0,
            DRM_SYNCOBJ_HANDLE_TO_FD_FLAGS_EXPORT_SYNC_FILE,
        );
        destroy(&self.inner.drm, handle);
        fd.map_err(DrmError::ExportSyncObj)
            .map(Rc::new)
            .map(SyncFile)
    }

    pub fn wait_for_point(
        &self,
        eventfd: &OwnedFd,
        sync_obj: &SyncObj,
        point: SyncObjPoint,
        signaled: bool,
    ) -> Result<(), DrmError> {
        let handle = self.get_handle(sync_obj)?;
        let flags = match signaled {
            true => 0,
            false => DRM_SYNCOBJ_WAIT_FLAGS_WAIT_AVAILABLE,
        };
        sync_obj_eventfd(
            self.inner.drm.raw(),
            eventfd.raw(),
            handle.0,
            point.0,
            flags,
        )
        .map_err(DrmError::RegisterEventfd)
    }

    pub fn supports_async_wait(&self) -> bool {
        self.supports_async_wait_().is_ok()
    }

    fn supports_async_wait_(&self) -> Result<(), DrmError> {
        let sync_obj = self.create_sync_obj()?;
        let eventfd = uapi::eventfd(0, c::EFD_CLOEXEC)
            .map_err(OsError::from)
            .map_err(DrmError::EventFd)?;
        self.wait_for_point(&eventfd, &sync_obj, SyncObjPoint(1), true)?;
        Ok(())
    }

    pub fn signal(&self, sync_obj: &SyncObj, point: SyncObjPoint) -> Result<(), DrmError> {
        let handle = self.get_handle(sync_obj)?;
        sync_obj_signal(self.inner.drm.raw(), handle.0, point.0).map_err(DrmError::SignalSyncObj)
    }

    pub fn import_sync_files<'a, I>(
        &self,
        sync_obj: &SyncObj,
        point: SyncObjPoint,
        sync_files: I,
    ) -> Result<(), DrmError>
    where
        I: IntoIterator<Item = &'a SyncFile>,
    {
        let mut sync_files = sync_files.into_iter();
        let Some(first) = sync_files.next() else {
            return self.signal(sync_obj, point);
        };
        let mut stash;
        let mut fd = &*first.0;
        for next in sync_files {
            stash = sync_ioc_merge(fd.raw(), next.raw()).map_err(DrmError::Merge)?;
            fd = &stash;
        }
        let dummy = self.get_dummy()?;
        sync_obj_fd_to_handle(
            self.inner.drm.raw(),
            fd.raw(),
            DRM_SYNCOBJ_FD_TO_HANDLE_FLAGS_IMPORT_SYNC_FILE,
            self.get_handle(&dummy)?.0,
        )
        .map_err(DrmError::ImportSyncFile)?;
        self.transfer(&dummy, SyncObjPoint(0), sync_obj, point)
    }

    fn transfer(
        &self,
        src_sync_obj: &SyncObj,
        src_point: SyncObjPoint,
        dst_sync_obj: &SyncObj,
        dst_point: SyncObjPoint,
    ) -> Result<(), DrmError> {
        let src_handle = self.get_handle(src_sync_obj)?;
        let dst_handle = self.get_handle(dst_sync_obj)?;
        sync_obj_transfer(
            self.inner.drm.raw(),
            src_handle.0,
            src_point.0,
            dst_handle.0,
            dst_point.0,
            0,
        )
        .map_err(DrmError::TransferPoint)
    }

    fn get_dummy(&self) -> Result<Rc<SyncObj>, DrmError> {
        match self.dummy.get() {
            Some(d) => Ok(d),
            None => {
                let d = Rc::new(self.create_sync_obj()?);
                self.dummy.set(Some(d.clone()));
                Ok(d)
            }
        }
    }
}

impl Drop for SyncObjCtx {
    fn drop(&mut self) {
        self.inner.links.clear();
        let mut map = self.inner.handles.lock();
        for handle in map.drain_values() {
            destroy(&self.inner.drm, handle);
        }
    }
}

fn destroy(drm: &OwnedFd, handle: SyncObjHandle) {
    if let Err(e) = sync_obj_destroy(drm.raw(), handle.0) {
        log::error!("Could not destroy sync obj: {}", ErrorFmt(e));
    }
}
