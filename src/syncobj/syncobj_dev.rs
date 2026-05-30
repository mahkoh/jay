use {
    crate::{
        gfx_api::SyncFile,
        syncobj::{
            SyncobjError,
            syncobj_dev::sys::{
                SYNCOBJ_CREATE_SIGNALED, SYNCOBJ_WAIT_FLAGS_WAIT_AVAILABLE,
                SYNCOBJ_WAIT_FLAGS_WAIT_FOR_SUBMIT, syncobj_create, syncobj_eventfd,
                syncobj_export_sync_file, syncobj_import_sync_file, syncobj_query, syncobj_signal,
                syncobj_wait,
            },
        },
        utils::{errorfmt::ErrorFmt, oserror::OsErrorExt},
        video::drm::syncobj::{Syncobj, SyncobjPoint},
    },
    std::{
        rc::Rc,
        sync::{Arc, LazyLock},
    },
    uapi::{OwnedFd, c},
};

mod sys;

pub struct SyncobjDev {
    dev: Arc<OwnedFd>,
}

impl SyncobjDev {
    pub fn get() -> Option<Self> {
        static DEV: LazyLock<Option<Arc<OwnedFd>>> = LazyLock::new(|| {
            let fd = uapi::open("/dev/syncobj", c::O_RDWR, 0);
            match fd.to_os_error() {
                Ok(f) => {
                    log::info!("Opened /dev/syncobj");
                    Some(Arc::new(f))
                }
                Err(e) => {
                    log::warn!("Could not open /dev/syncobj: {}", ErrorFmt(e));
                    None
                }
            }
        });
        DEV.clone().map(|fd| Self { dev: fd })
    }

    pub fn create(&self) -> Result<Syncobj, SyncobjError> {
        let fd = syncobj_create(self.dev.raw(), 0).map_err(SyncobjError::CreateSyncobj)?;
        Ok(Syncobj::new(&Rc::new(fd)))
    }

    pub fn create_signaled_sync_file(&self) -> Result<SyncFile, SyncobjError> {
        let fd = syncobj_create(self.dev.raw(), SYNCOBJ_CREATE_SIGNALED)
            .map_err(SyncobjError::CreateSyncobj)?;
        let fd = syncobj_export_sync_file(self.dev.raw(), fd.raw(), 0)
            .map_err(SyncobjError::ExportSyncFile)?;
        Ok(SyncFile(Rc::new(fd)))
    }

    pub fn wait_for_point(
        &self,
        eventfd: &OwnedFd,
        syncobj: &Syncobj,
        point: SyncobjPoint,
        signaled: bool,
    ) -> Result<(), SyncobjError> {
        let flags = match signaled {
            true => 0,
            false => SYNCOBJ_WAIT_FLAGS_WAIT_AVAILABLE,
        };
        syncobj_eventfd(
            self.dev.raw(),
            eventfd.raw(),
            syncobj.fd().raw(),
            point.0,
            flags,
        )
        .map_err(SyncobjError::RegisterEventfd)
    }

    pub fn import_sync_file(
        &self,
        syncobj: &Syncobj,
        point: SyncobjPoint,
        sync_file: &SyncFile,
    ) -> Result<(), SyncobjError> {
        syncobj_import_sync_file(
            self.dev.raw(),
            syncobj.fd().raw(),
            point.0,
            sync_file.0.raw(),
        )
        .map_err(SyncobjError::ImportSyncFile)
    }

    pub fn export_sync_file_blocking(
        &self,
        syncobj: &Syncobj,
        point: SyncobjPoint,
    ) -> Result<SyncFile, SyncobjError> {
        let export = |syncobj: &Syncobj, point: SyncobjPoint| {
            syncobj_export_sync_file(self.dev.raw(), syncobj.fd().raw(), point.0)
                .map(Rc::new)
                .map(SyncFile)
        };
        let res = export(syncobj, point);
        match res {
            Ok(sf) => return Ok(sf),
            Err(e) if e.0 == c::EINVAL => {}
            Err(e) => return Err(SyncobjError::ExportSyncFile(e)),
        }
        syncobj_wait(
            self.dev.raw(),
            syncobj.fd().raw(),
            point.0,
            SYNCOBJ_WAIT_FLAGS_WAIT_FOR_SUBMIT | SYNCOBJ_WAIT_FLAGS_WAIT_AVAILABLE,
        )
        .map_err(SyncobjError::WaitForPoint)?;
        export(syncobj, point).map_err(SyncobjError::ExportSyncFile)
    }

    pub fn query_last_signaled(&self, syncobj: &Syncobj) -> Result<u64, SyncobjError> {
        syncobj_query(self.dev.raw(), syncobj.fd().raw()).map_err(SyncobjError::QuerySyncobj)
    }

    pub fn signal(&self, syncobj: &Syncobj, point: SyncobjPoint) -> Result<(), SyncobjError> {
        syncobj_signal(self.dev.raw(), syncobj.fd().raw(), point.0)
            .map_err(SyncobjError::SignalSyncobj)
    }
}
