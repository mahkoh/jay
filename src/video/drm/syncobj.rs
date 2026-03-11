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
            DrmError, NodeType, get_drm_nodes_from_dev,
            sys::{
                DRM_SYNCOBJ_CREATE_SIGNALED, DRM_SYNCOBJ_FD_TO_HANDLE_FLAGS_IMPORT_SYNC_FILE,
                DRM_SYNCOBJ_FD_TO_HANDLE_FLAGS_TIMELINE,
                DRM_SYNCOBJ_HANDLE_TO_FD_FLAGS_EXPORT_SYNC_FILE,
                DRM_SYNCOBJ_HANDLE_TO_FD_FLAGS_TIMELINE, DRM_SYNCOBJ_WAIT_FLAGS_WAIT_AVAILABLE,
                DRM_SYNCOBJ_WAIT_FLAGS_WAIT_FOR_SUBMIT, sync_ioc_merge, syncobj_create,
                syncobj_destroy, syncobj_eventfd, syncobj_fd_to_handle, syncobj_handle_to_fd,
                syncobj_query, syncobj_signal, syncobj_transfer,
            },
        },
    },
    std::{
        cell::OnceCell,
        rc::Rc,
        sync::atomic::{AtomicU64, Ordering::Relaxed},
    },
    uapi::{OwnedFd, c},
};

static SYNCOBJ_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct SyncobjId(u64);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct SyncobjHandle(u32);

unsafe impl UnsafeCellCloneSafe for SyncobjHandle {}

pub struct Syncobj {
    id: SyncobjId,
    fd: Rc<OwnedFd>,
    importers: LinkedList<Rc<Handles>>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct SyncobjPoint(pub u64);

impl Syncobj {
    pub fn new(fd: &Rc<OwnedFd>) -> Self {
        Self {
            id: SyncobjId(SYNCOBJ_ID.fetch_add(1, Relaxed)),
            fd: fd.clone(),
            importers: Default::default(),
        }
    }

    #[cfg_attr(not(feature = "it"), expect(dead_code))]
    pub fn fd(&self) -> &Rc<OwnedFd> {
        &self.fd
    }

    pub fn id(&self) -> SyncobjId {
        self.id
    }
}

impl Drop for Syncobj {
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
    handles: CopyHashMap<SyncobjId, SyncobjHandle>,
    links: CopyHashMap<SyncobjId, LinkedNode<Rc<Handles>>>,
}

pub struct SyncobjCtx {
    inner: Rc<Handles>,
    dummy: CloneCell<Option<Rc<Syncobj>>>,
    supports_timeline_import: OnceCell<bool>,
}

impl SyncobjCtx {
    pub fn new(drm: &Rc<OwnedFd>) -> Self {
        Self {
            inner: Rc::new(Handles {
                drm: drm.clone(),
                handles: Default::default(),
                links: Default::default(),
            }),
            dummy: Default::default(),
            supports_timeline_import: Default::default(),
        }
    }

    pub fn from_dev_t(dev: c::dev_t) -> Result<Self, DrmError> {
        let nodes = get_drm_nodes_from_dev(uapi::major(dev), uapi::minor(dev))
            .map_err(DrmError::GetNodes)?;
        let path = nodes
            .get(&NodeType::Render)
            .or_else(|| nodes.get(&NodeType::Primary))
            .ok_or(DrmError::NoDeviceNodes)?;
        let device_fd = uapi::open(path.as_c_str(), c::O_RDWR | c::O_CLOEXEC, 0)
            .map(Rc::new)
            .map_err(Into::into)
            .map_err(DrmError::ReopenNode)?;
        Ok(Self::new(&device_fd))
    }

    fn get_handle(&self, syncobj: &Syncobj) -> Result<SyncobjHandle, DrmError> {
        if let Some(handle) = self.inner.handles.get(&syncobj.id) {
            return Ok(handle);
        }
        let handle = syncobj_fd_to_handle(self.inner.drm.raw(), syncobj.fd.raw(), 0, 0, 0)
            .map_err(DrmError::ImportSyncobj)?;
        let handle = SyncobjHandle(handle);
        let link = syncobj.importers.add_last(self.inner.clone());
        self.inner.handles.set(syncobj.id, handle);
        self.inner.links.set(syncobj.id, link);
        Ok(handle)
    }

    pub fn create_syncobj(&self) -> Result<Syncobj, DrmError> {
        let handle = syncobj_create(self.inner.drm.raw(), 0).map_err(DrmError::CreateSyncobj)?;
        let handle = SyncobjHandle(handle);
        let fd = syncobj_handle_to_fd(self.inner.drm.raw(), handle.0, 0, 0);
        if fd.is_err() {
            destroy(&self.inner.drm, handle);
        }
        let fd = fd.map_err(DrmError::ExportSyncobj).map(Rc::new)?;
        let syncobj = Syncobj::new(&fd);
        let link = syncobj.importers.add_last(self.inner.clone());
        self.inner.handles.set(syncobj.id, handle);
        self.inner.links.set(syncobj.id, link);
        Ok(syncobj)
    }

    pub fn create_signaled_sync_file(&self) -> Result<SyncFile, DrmError> {
        let handle = syncobj_create(self.inner.drm.raw(), DRM_SYNCOBJ_CREATE_SIGNALED)
            .map_err(DrmError::CreateSyncobj)?;
        let handle = SyncobjHandle(handle);
        let fd = syncobj_handle_to_fd(
            self.inner.drm.raw(),
            handle.0,
            DRM_SYNCOBJ_HANDLE_TO_FD_FLAGS_EXPORT_SYNC_FILE,
            0,
        );
        destroy(&self.inner.drm, handle);
        fd.map_err(DrmError::ExportSyncobj)
            .map(Rc::new)
            .map(SyncFile)
    }

    pub fn wait_for_point(
        &self,
        eventfd: &OwnedFd,
        syncobj: &Syncobj,
        point: SyncobjPoint,
        signaled: bool,
    ) -> Result<(), DrmError> {
        let handle = self.get_handle(syncobj)?;
        let flags = match signaled {
            true => 0,
            false => DRM_SYNCOBJ_WAIT_FLAGS_WAIT_AVAILABLE,
        };
        syncobj_eventfd(
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
        let syncobj = self.create_syncobj()?;
        let eventfd = uapi::eventfd(0, c::EFD_CLOEXEC)
            .map_err(OsError::from)
            .map_err(DrmError::EventFd)?;
        self.wait_for_point(&eventfd, &syncobj, SyncobjPoint(1), true)?;
        Ok(())
    }

    fn supports_timeline_import(&self) -> bool {
        *self
            .supports_timeline_import
            .get_or_init(|| match self.test_timeline_import() {
                Ok(_) => {
                    log::info!("Kernel supports sync file timeline import");
                    true
                }
                Err(e) => {
                    log::warn!(
                        "Kernel does not support sync file timeline import: {}",
                        ErrorFmt(e),
                    );
                    false
                }
            })
    }

    fn test_timeline_import(&self) -> Result<(), DrmError> {
        let syncobj = self.create_syncobj()?;
        let syncobj = self.get_handle(&syncobj)?;
        let sync_file = self.create_signaled_sync_file()?;
        syncobj_fd_to_handle(
            self.inner.drm.raw(),
            sync_file.raw(),
            DRM_SYNCOBJ_FD_TO_HANDLE_FLAGS_IMPORT_SYNC_FILE
                | DRM_SYNCOBJ_FD_TO_HANDLE_FLAGS_TIMELINE,
            syncobj.0,
            123,
        )
        .map(drop)
        .map_err(DrmError::ImportSyncFile)
    }

    pub fn signal(&self, syncobj: &Syncobj, point: SyncobjPoint) -> Result<(), DrmError> {
        let handle = self.get_handle(syncobj)?;
        syncobj_signal(self.inner.drm.raw(), handle.0, point.0).map_err(DrmError::SignalSyncobj)
    }

    pub fn import_sync_files<'a, I>(
        &self,
        syncobj: &Syncobj,
        point: SyncobjPoint,
        sync_files: I,
    ) -> Result<(), DrmError>
    where
        I: IntoIterator<Item = &'a SyncFile>,
    {
        let Some(fd) = merge_sync_files(sync_files)? else {
            return self.signal(syncobj, point);
        };
        let import = |flags: u32, handle: SyncobjHandle| {
            syncobj_fd_to_handle(
                self.inner.drm.raw(),
                fd.raw(),
                DRM_SYNCOBJ_FD_TO_HANDLE_FLAGS_IMPORT_SYNC_FILE | flags,
                handle.0,
                point.0,
            )
            .map(drop)
            .map_err(DrmError::ImportSyncFile)
        };
        if self.supports_timeline_import() {
            return import(
                DRM_SYNCOBJ_FD_TO_HANDLE_FLAGS_TIMELINE,
                self.get_handle(syncobj)?,
            );
        }
        let dummy = self.get_dummy()?;
        import(0, self.get_handle(&dummy)?)?;
        self.transfer(&dummy, SyncobjPoint(0), syncobj, point, 0)
    }

    pub fn export_sync_file_blocking(
        &self,
        syncobj: &Syncobj,
        point: SyncobjPoint,
    ) -> Result<SyncFile, DrmError> {
        let export = |flags: u32, handle: SyncobjHandle, point: SyncobjPoint| {
            syncobj_handle_to_fd(
                self.inner.drm.raw(),
                handle.0,
                DRM_SYNCOBJ_HANDLE_TO_FD_FLAGS_EXPORT_SYNC_FILE | flags,
                point.0,
            )
            .map(Rc::new)
            .map(SyncFile)
        };
        if self.supports_timeline_import() {
            let res = export(
                DRM_SYNCOBJ_HANDLE_TO_FD_FLAGS_TIMELINE,
                self.get_handle(syncobj)?,
                point,
            );
            match res {
                Ok(sf) => return Ok(sf),
                Err(e) if e.0 == c::EINVAL => {}
                Err(e) => return Err(DrmError::ExportSyncFile(e)),
            }
        }
        let dummy = self.get_dummy()?;
        let zero = SyncobjPoint(0);
        self.transfer(
            syncobj,
            point,
            &dummy,
            zero,
            DRM_SYNCOBJ_WAIT_FLAGS_WAIT_FOR_SUBMIT,
        )?;
        export(0, self.get_handle(&dummy)?, zero).map_err(DrmError::ExportSyncFile)
    }

    fn transfer(
        &self,
        src_syncobj: &Syncobj,
        src_point: SyncobjPoint,
        dst_syncobj: &Syncobj,
        dst_point: SyncobjPoint,
        flags: u32,
    ) -> Result<(), DrmError> {
        let src_handle = self.get_handle(src_syncobj)?;
        let dst_handle = self.get_handle(dst_syncobj)?;
        syncobj_transfer(
            self.inner.drm.raw(),
            src_handle.0,
            src_point.0,
            dst_handle.0,
            dst_point.0,
            flags,
        )
        .map_err(DrmError::TransferPoint)
    }

    fn get_dummy(&self) -> Result<Rc<Syncobj>, DrmError> {
        match self.dummy.get() {
            Some(d) => Ok(d),
            None => {
                let d = Rc::new(self.create_syncobj()?);
                self.dummy.set(Some(d.clone()));
                Ok(d)
            }
        }
    }

    pub fn query_last_signaled(&self, syncobj: &Syncobj) -> Result<u64, DrmError> {
        let handle = self.get_handle(syncobj)?;
        syncobj_query(self.inner.drm.raw(), handle.0).map_err(DrmError::QuerySyncobj)
    }
}

impl Drop for SyncobjCtx {
    fn drop(&mut self) {
        self.inner.links.clear();
        let mut map = self.inner.handles.lock();
        for handle in map.drain_values() {
            destroy(&self.inner.drm, handle);
        }
    }
}

fn destroy(drm: &OwnedFd, handle: SyncobjHandle) {
    if let Err(e) = syncobj_destroy(drm.raw(), handle.0) {
        log::error!("Could not destroy syncobj: {}", ErrorFmt(e));
    }
}

pub fn merge_sync_files<'a, I>(sync_files: I) -> Result<Option<SyncFile>, DrmError>
where
    I: IntoIterator<Item = &'a SyncFile>,
{
    let mut sync_files = sync_files.into_iter();
    let Some(first) = sync_files.next() else {
        return Ok(None);
    };
    let Some(second) = sync_files.next() else {
        return Ok(Some(first.clone()));
    };
    let merge = |left: &OwnedFd, right: &OwnedFd| {
        sync_ioc_merge(left.raw(), right.raw()).map_err(DrmError::Merge)
    };
    let mut fd = merge(first, second)?;
    for next in sync_files {
        fd = merge(&fd, next)?;
    }
    Ok(Some(SyncFile(Rc::new(fd))))
}
