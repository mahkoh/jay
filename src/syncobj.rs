use crate::gfx_api::SyncFile;
use crate::io_uring::IoUringError;
use crate::utils::oserror::OsError;
use crate::video::drm::DrmError;
use crate::video::drm::syncobj::DrmSyncobjCtx;
use crate::video::drm::syncobj::Syncobj;
use crate::video::drm::syncobj::SyncobjPoint;
use crate::video::drm::syncobj::merge_sync_files;
use std::rc::Rc;
use thiserror::Error;
use uapi::OwnedFd;
use uapi::c;

pub mod wait_for_syncobj;

#[derive(Debug, Error)]
pub enum SyncobjError {
    #[error(transparent)]
    DrmError(#[from] DrmError),
    #[error("Could not create a syncobj")]
    CreateSyncobj(#[source] OsError),
    #[error("Could not export a sync file")]
    ExportSyncFile(#[source] OsError),
    #[error("Could not import a sync file")]
    ImportSyncFile(#[source] OsError),
    #[error("Could not register an eventfd")]
    RegisterEventfd(#[source] OsError),
    #[error("Could not query syncobj")]
    QuerySyncobj(#[source] OsError),
    #[error("Could not signal a syncobj")]
    SignalSyncobj(#[source] OsError),
    #[error("No syncobj context available")]
    NoSyncobjContextAvailable,
    #[error("Could not create an eventfd")]
    EventFd(#[source] OsError),
    #[error("Could not read from an eventfd")]
    ReadEventFd(#[source] IoUringError),
}

pub struct SyncobjCtx {
    backend: SyncobjBackend,
}

enum SyncobjBackend {
    Drm(DrmSyncobjCtx),
}

impl SyncobjCtx {
    pub fn new(drm: &Rc<OwnedFd>) -> Self {
        Self {
            backend: SyncobjBackend::Drm(DrmSyncobjCtx::new(drm)),
        }
    }

    pub fn from_dev_t(dev: c::dev_t) -> Result<Self, SyncobjError> {
        Ok(Self {
            backend: SyncobjBackend::Drm(DrmSyncobjCtx::from_dev_t(dev)?),
        })
    }

    #[cfg_attr(not(feature = "it"), expect(dead_code))]
    pub fn create_syncobj(&self) -> Result<Syncobj, SyncobjError> {
        match &self.backend {
            SyncobjBackend::Drm(b) => b.create_syncobj(),
        }
    }

    pub fn create_signaled_sync_file(&self) -> Result<SyncFile, SyncobjError> {
        match &self.backend {
            SyncobjBackend::Drm(b) => b.create_signaled_sync_file(),
        }
    }

    pub fn wait_for_point(
        &self,
        eventfd: &OwnedFd,
        syncobj: &Syncobj,
        point: SyncobjPoint,
        signaled: bool,
    ) -> Result<(), SyncobjError> {
        match &self.backend {
            SyncobjBackend::Drm(b) => b.wait_for_point(eventfd, syncobj, point, signaled),
        }
    }

    pub fn supports_async_wait(&self) -> bool {
        match &self.backend {
            SyncobjBackend::Drm(b) => b.supports_async_wait(),
        }
    }

    pub fn signal(&self, syncobj: &Syncobj, point: SyncobjPoint) -> Result<(), SyncobjError> {
        match &self.backend {
            SyncobjBackend::Drm(b) => b.signal(syncobj, point),
        }
    }

    pub fn import_sync_files<'a, I>(
        &self,
        syncobj: &Syncobj,
        point: SyncobjPoint,
        sync_files: I,
    ) -> Result<(), SyncobjError>
    where
        I: IntoIterator<Item = &'a SyncFile>,
    {
        let Some(fd) = merge_sync_files(sync_files)? else {
            return self.signal(syncobj, point);
        };
        match &self.backend {
            SyncobjBackend::Drm(b) => b.import_sync_file(syncobj, point, &fd),
        }
    }

    pub fn export_sync_file_blocking(
        &self,
        syncobj: &Syncobj,
        point: SyncobjPoint,
    ) -> Result<SyncFile, SyncobjError> {
        match &self.backend {
            SyncobjBackend::Drm(b) => b.export_sync_file_blocking(syncobj, point),
        }
    }

    pub fn query_last_signaled(&self, syncobj: &Syncobj) -> Result<u64, SyncobjError> {
        match &self.backend {
            SyncobjBackend::Drm(b) => b.query_last_signaled(syncobj),
        }
    }
}
