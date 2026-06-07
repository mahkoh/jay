use {
    crate::{
        copy_device::{CopyDeviceCopy, CopyDeviceDstObject, CopyDeviceError, CopyDeviceSrcObject},
        format::Format,
        gfx_api::{FdSync, SyncFile},
        io_uring::{IoUring, IoUringError, PendingPoll, PollCallback},
        rect::Region,
        state::{DrmDevData, State},
        utils::{compat::IoctlNumber, errorfmt::ErrorFmt, numcell::NumCell, oserror::OsError},
        video::{
            LINEAR_MODIFIER, Modifier,
            drm::{DrmError, syncobj::merge_sync_files},
        },
    },
    arrayvec::ArrayVec,
    bstr::ByteSlice,
    smallvec::SmallVec,
    std::{
        cell::{Cell, OnceCell, RefCell},
        ffi::c_short,
        io::Read,
        rc::Rc,
        sync::OnceLock,
    },
    thiserror::Error,
    uapi::{
        _IOW, _IOWR, OwnedFd,
        c::{self, dev_t, ioctl},
        format_ustr,
    },
};

#[derive(Clone, Debug)]
pub struct DmaBufPlane {
    pub offset: u32,
    pub stride: u32,
    pub fd: Rc<OwnedFd>,
}

linear_ids!(DmaBufIds, DmaBufId);

#[derive(Debug)]
pub struct DmaBuf {
    pub id: DmaBufId,
    pub width: i32,
    pub height: i32,
    pub format: &'static Format,
    pub modifier: Modifier,
    pub planes: PlaneVec<DmaBufPlane>,
    is_disjoint: OnceCell<bool>,
}

pub const MAX_PLANES: usize = 4;

pub type PlaneVec<T> = ArrayVec<T, MAX_PLANES>;

impl DmaBuf {
    pub fn new(
        ids: &DmaBufIds,
        width: i32,
        height: i32,
        format: &'static Format,
        modifier: Modifier,
        planes: PlaneVec<DmaBufPlane>,
    ) -> Rc<Self> {
        Rc::new(Self {
            id: ids.next(),
            width,
            height,
            format,
            modifier,
            planes,
            is_disjoint: Default::default(),
        })
    }

    pub fn is_disjoint(&self) -> bool {
        *self.is_disjoint.get_or_init(|| {
            if self.planes.len() <= 1 {
                return false;
            }
            let stat = match uapi::fstat(self.planes[0].fd.raw()) {
                Ok(s) => s,
                _ => return true,
            };
            for plane in &self.planes[1..] {
                let stat2 = match uapi::fstat(plane.fd.raw()) {
                    Ok(s) => s,
                    _ => return true,
                };
                if stat2.st_ino != stat.st_ino {
                    return true;
                }
            }
            false
        })
    }

    pub fn is_one_file(&self) -> bool {
        !self.is_disjoint()
    }

    pub fn udmabuf_size(&self) -> Option<usize> {
        if self.planes.len() != 1 {
            return None;
        }
        if self.modifier != LINEAR_MODIFIER {
            return None;
        }
        let stat = match uapi::fstat(self.planes[0].fd.raw()) {
            Ok(s) => s,
            _ => return None,
        };
        static DMABUF_DEV: OnceLock<dev_t> = OnceLock::new();
        match DMABUF_DEV.get() {
            Some(d) => {
                if stat.st_dev != *d {
                    return None;
                }
            }
            None => {
                if dma_buf_export_sync_file(&self.planes[0].fd, DMA_BUF_SYNC_READ).is_err() {
                    return None;
                }
                let _ = DMABUF_DEV.set(stat.st_dev);
            }
        }
        if is_not_udmabuf(&self.planes[0].fd, stat.st_ino) {
            return None;
        }
        Some(stat.st_size as usize)
    }

    pub fn import_sync_file(&self, flags: u32, sync_file: &OwnedFd) -> Result<(), OsError> {
        for plane in &self.planes {
            dma_buf_import_sync_file(&plane.fd, flags, sync_file)?;
            if self.is_one_file() {
                break;
            }
        }
        Ok(())
    }

    pub fn export_sync_file(&self, flags: u32) -> Result<Option<SyncFile>, DrmError> {
        let mut sf = PlaneVec::new();
        for plane in &self.planes {
            sf.push(
                dma_buf_export_sync_file(&plane.fd, flags)
                    .map(Rc::new)
                    .map(SyncFile)
                    .map_err(DrmError::ExportSyncFile)?,
            );
            if self.is_one_file() {
                break;
            }
        }
        merge_sync_files(sf.iter())
    }
}

const DMA_BUF_BASE: u64 = b'b' as _;

#[repr(C)]
struct dma_buf_export_sync_file {
    flags: u32,
    fd: i32,
}

#[repr(C)]
struct dma_buf_import_sync_file {
    flags: u32,
    fd: i32,
}

pub const DMA_BUF_SYNC_READ: u32 = 1 << 0;
pub const DMA_BUF_SYNC_WRITE: u32 = 1 << 1;

const DMA_BUF_IOCTL_EXPORT_SYNC_FILE: IoctlNumber =
    _IOWR::<dma_buf_export_sync_file>(DMA_BUF_BASE, 2) as IoctlNumber;
const DMA_BUF_IOCTL_IMPORT_SYNC_FILE: IoctlNumber =
    _IOW::<dma_buf_import_sync_file>(DMA_BUF_BASE, 3) as IoctlNumber;

pub fn dma_buf_export_sync_file(dmabuf: &OwnedFd, flags: u32) -> Result<OwnedFd, OsError> {
    let mut data = dma_buf_export_sync_file { flags, fd: -1 };
    let res = unsafe { ioctl(dmabuf.raw(), DMA_BUF_IOCTL_EXPORT_SYNC_FILE, &mut data) };
    if res != 0 {
        Err(OsError::default())
    } else {
        Ok(OwnedFd::new(data.fd))
    }
}

pub fn dma_buf_import_sync_file(
    dmabuf: &OwnedFd,
    flags: u32,
    sync_file: &OwnedFd,
) -> Result<(), OsError> {
    let mut data = dma_buf_import_sync_file {
        flags,
        fd: sync_file.raw(),
    };
    let res = unsafe { ioctl(dmabuf.raw(), DMA_BUF_IOCTL_IMPORT_SYNC_FILE, &mut data) };
    if res != 0 {
        Err(OsError::default())
    } else {
        Ok(())
    }
}

fn is_not_udmabuf(fd: &OwnedFd, ino: c::ino_t) -> bool {
    !is_udmabuf(fd, ino)
}

fn is_udmabuf(fd: &OwnedFd, ino: c::ino_t) -> bool {
    {
        thread_local! {
            static BUF: *mut Vec<u8> = Box::into_raw(Box::new(vec!()));
        }
        let buf = BUF.with(|b| *b);
        let buf = unsafe { &mut *buf };
        buf.clear();
        let path = format_ustr!("/proc/self/fdinfo/{}", fd.raw());
        if let Ok(mut file) = uapi::open(path, c::O_RDONLY, 0)
            && let Ok(_) = file.read_to_end(buf)
        {
            for line in buf.split_str(b"\n") {
                if let Some(v) = line.strip_prefix(b"exp_name:") {
                    return v.trim_ascii() == b"udmabuf";
                }
            }
        }
    }
    {
        let path = format_ustr!("/sys/kernel/dmabuf/buffers/{ino}/exporter_name");
        const MARKER: &[u8] = b"udmabuf\n";
        let mut buf = [0u8; MARKER.len()];
        if let Ok(file) = uapi::open(path, c::O_RDONLY, 0)
            && let Ok(_) = uapi::read(file.raw(), &mut buf)
            && buf == MARKER
        {
            return true;
        }
    }
    false
}

impl State {
    pub fn find_dmabuf_device(
        &self,
        buf: &DmaBuf,
        hint_dev: Option<&Rc<DrmDevData>>,
    ) -> Option<Rc<DrmDevData>> {
        let is_on_device = |dev: &Rc<DrmDevData>| {
            let Some(cd) = &dev.id_device else {
                return false;
            };
            cd.is_on_device(buf)
                .inspect_err(|e| {
                    log::warn!("Could not check if dmabuf is on device: {}", ErrorFmt(&**e));
                })
                .unwrap_or(false)
        };
        let mut hint_dev_id = None;
        if let Some(dev) = hint_dev {
            hint_dev_id = Some(dev.id);
            if is_on_device(&dev) {
                return Some(dev.clone());
            }
        }
        let render_dev_id = self.render_ctx_drm_device.id();
        if render_dev_id != hint_dev_id
            && let Some(dev) = self.render_ctx_drm_device.get()
            && is_on_device(&dev)
        {
            return Some(dev);
        }
        for dev in self.drm_devs.lock().values() {
            if render_dev_id == Some(dev.id) || hint_dev_id == Some(dev.id) {
                continue;
            }
            if is_on_device(dev) {
                return Some(dev.clone());
            }
        }
        None
    }
}

#[derive(Debug, Error)]
pub enum ChainedCopyError {
    #[error("Could not schedule copy")]
    Copy(#[source] CopyDeviceError),
    #[error("Could not schedule poll")]
    SchedulePoll(#[source] IoUringError),
    #[error("Could not wait for poll to complete")]
    AwaitPoll(#[source] OsError),
}

pub trait ChainedCopyCallback {
    fn completed(self: Rc<Self>, res: Result<(), ChainedCopyError>);
}

#[derive(Clone)]
pub enum DmabufCopy {
    #[expect(dead_code)]
    Fixed(Rc<CopyDeviceCopy>),
    AdHoc(Rc<CopyDeviceSrcObject>, Rc<CopyDeviceDstObject>),
}

pub struct PendingChainedCopy {
    copy: Rc<ChainedCopy>,
}

struct ChainedCopy {
    ring: Rc<IoUring>,
    offset: NumCell<usize>,
    steps: SmallVec<[DmabufCopy; 2]>,
    cb: Cell<Option<Rc<dyn ChainedCopyCallback>>>,
    poll: Cell<Option<PendingPoll>>,
    region: Option<Region>,
    sync: RefCell<SmallVec<[FdSync; 1]>>,
}

impl Drop for PendingChainedCopy {
    fn drop(&mut self) {
        self.copy.cb.take();
        self.copy.poll.take();
    }
}

impl PollCallback for ChainedCopy {
    fn completed(self: Rc<Self>, res: Result<c_short, OsError>) {
        self.poll.take();
        if let Err(e) = res {
            self.done(Err(ChainedCopyError::AwaitPoll(e)));
            return;
        }
        match self.run() {
            Ok(true) => self.done(Ok(())),
            Ok(false) => {}
            Err(e) => self.done(Err(e)),
        }
    }
}

impl DmabufCopy {
    pub fn execute(
        &self,
        sync: Option<&FdSync>,
        region: Option<&Region>,
    ) -> Result<Option<FdSync>, CopyDeviceError> {
        match self {
            DmabufCopy::Fixed(c) => c.execute(sync, region),
            DmabufCopy::AdHoc(src, dst) => src.copy_to(dst, sync, region),
        }
    }
}

impl ChainedCopy {
    fn done(&self, res: Result<(), ChainedCopyError>) {
        if let Some(cb) = self.cb.take() {
            cb.completed(res);
        }
    }

    fn run(self: &Rc<Self>) -> Result<bool, ChainedCopyError> {
        let sync = &mut *self.sync.borrow_mut();
        loop {
            while sync.is_empty() {
                let offset = self.offset.fetch_add(1);
                let Some(step) = self.steps.get(offset) else {
                    return Ok(true);
                };
                match step.execute(None, self.region.as_ref()) {
                    Ok(s) => sync.extend(s),
                    Err(e) => {
                        return Err(ChainedCopyError::Copy(e));
                    }
                }
            }
            let Some(sync) = sync.pop() else {
                return Ok(true);
            };
            match sync.signaled_external(&self.ring, self.clone()) {
                Ok(Some(pending)) => {
                    self.poll.set(Some(pending));
                    return Ok(false);
                }
                Ok(None) => {}
                Err(e) => {
                    return Err(ChainedCopyError::SchedulePoll(e));
                }
            }
        }
    }
}

impl State {
    pub fn schedule_chained_copy(
        &self,
        steps: &[DmabufCopy],
        cb: Rc<dyn ChainedCopyCallback>,
        sync: SmallVec<[FdSync; 1]>,
        region: Option<Region>,
    ) -> Result<Option<PendingChainedCopy>, ChainedCopyError> {
        let copy = Rc::new(ChainedCopy {
            ring: self.ring.clone(),
            offset: Default::default(),
            steps: steps.iter().cloned().collect(),
            cb: Cell::new(Some(cb)),
            poll: Default::default(),
            region,
            sync: RefCell::new(sync),
        });
        if copy.run()? {
            return Ok(None);
        }
        Ok(Some(PendingChainedCopy { copy }))
    }
}
