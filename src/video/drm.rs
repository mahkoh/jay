pub mod sync_obj;
mod sys;
pub mod wait_for_sync_obj;
pub use consts::*;

use {
    crate::{
        utils::oserror::OsError,
        video::drm::sys::{
            DRM_DISPLAY_MODE_LEN, DRM_MODE_ATOMIC_TEST_ONLY, DRM_MODE_FB_MODIFIERS,
            DRM_MODE_OBJECT_BLOB, DRM_MODE_OBJECT_CONNECTOR, DRM_MODE_OBJECT_CRTC,
            DRM_MODE_OBJECT_ENCODER, DRM_MODE_OBJECT_FB, DRM_MODE_OBJECT_MODE,
            DRM_MODE_OBJECT_PLANE, DRM_MODE_OBJECT_PROPERTY, create_lease, drm_event,
            drm_event_vblank, gem_close, get_cap, get_device_name_from_fd2, get_minor_name_from_fd,
            get_node_type_from_fd, get_nodes, mode_addfb2, mode_atomic, mode_create_blob,
            mode_destroy_blob, mode_get_resources, mode_getconnector, mode_getencoder,
            mode_getplane, mode_getplaneresources, mode_getprobblob, mode_getproperty,
            mode_obj_getproperties, mode_rmfb, prime_fd_to_handle, set_client_cap,
        },
    },
    ahash::AHashMap,
    bstr::{BString, ByteSlice},
    indexmap::IndexSet,
    std::{
        cell::{Cell, RefCell},
        ffi::CString,
        fmt::{Debug, Display, Formatter},
        mem::{self, MaybeUninit},
        ops::Deref,
        rc::{Rc, Weak},
    },
    thiserror::Error,
    uapi::{OwnedFd, Pod, Ustring, c},
};

use crate::{
    backend,
    format::Format,
    io_uring::{IoUring, IoUringError},
    utils::{buf::Buf, errorfmt::ErrorFmt, stack::Stack, syncqueue::SyncQueue, vec_ext::VecExt},
    video::{
        INVALID_MODIFIER, Modifier,
        dmabuf::DmaBuf,
        drm::sys::{
            DRM_CAP_ATOMIC_ASYNC_PAGE_FLIP, DRM_CAP_CURSOR_HEIGHT, DRM_CAP_CURSOR_WIDTH,
            FORMAT_BLOB_CURRENT, auth_magic, drm_event_crtc_sequence, drm_format_modifier,
            drm_format_modifier_blob, drop_master, get_version, queue_sequence, revoke_lease,
        },
    },
};
pub use sys::{
    DRM_CLIENT_CAP_ATOMIC, DRM_MODE_ATOMIC_ALLOW_MODESET, DRM_MODE_ATOMIC_NONBLOCK,
    DRM_MODE_PAGE_FLIP_ASYNC, DRM_MODE_PAGE_FLIP_EVENT, drm_mode_modeinfo,
};

#[derive(Debug, Error)]
pub enum DrmError {
    #[error("Could not reopen a node")]
    ReopenNode(#[source] OsError),
    #[error("Could not retrieve the render node name")]
    RenderNodeName(#[source] OsError),
    #[error("Could not retrieve the device node name")]
    DeviceNodeName(#[source] OsError),
    #[error("Could not retrieve device nodes")]
    GetNodes(#[source] OsError),
    #[error("Could not retrieve device type")]
    GetDeviceType(#[source] OsError),
    #[error("Could not perform drm property ioctl")]
    GetProperty(#[source] OsError),
    #[error("Could not perform drm getencoder ioctl")]
    GetEncoder(#[source] OsError),
    #[error("Could not perform drm getresources ioctl")]
    GetResources(#[source] OsError),
    #[error("Could not perform drm getplaneresources ioctl")]
    GetPlaneResources(#[source] OsError),
    #[error("Could not perform drm getplane ioctl")]
    GetPlane(#[source] OsError),
    #[error("Could not create a blob")]
    CreateBlob(#[source] OsError),
    #[error("Could not perform drm getconnector ioctl")]
    GetConnector(#[source] OsError),
    #[error("Could not perform drm getprobblob ioctl")]
    GetPropBlob(#[source] OsError),
    #[error("Property has an invalid size")]
    InvalidProbSize,
    #[error("Property has a size that is not a multiple of the vector type")]
    UnalignedPropSize,
    #[error("Could not perform drm properties ioctl")]
    GetProperties(#[source] OsError),
    #[error("Could not perform drm atomic ioctl")]
    Atomic(#[source] OsError),
    #[error("Could not inspect a connector")]
    CreateConnector(#[source] Box<DrmError>),
    #[error("Drm property has an unknown type {0}")]
    UnknownPropertyType(u32),
    #[error("Range property does not have exactly two values")]
    RangeValues,
    #[error("Object property does not have exactly one value")]
    ObjectValues,
    #[error("Object does not have the required property {0}")]
    MissingProperty(Box<str>),
    #[error("Plane has an unknown type {0}")]
    UnknownPlaneType(BString),
    #[error("Plane has an invalid type {0}")]
    InvalidPlaneType(u64),
    #[error("Plane type property has an invalid property type")]
    InvalidPlaneTypeProperty,
    #[error("Could not create a framebuffer")]
    AddFb(#[source] OsError),
    #[error("Could not convert prime fd to gem handle")]
    GemHandle(#[source] OsError),
    #[error("Could not read events from the drm fd")]
    ReadEvents(#[source] IoUringError),
    #[error("Read invalid data from drm device")]
    InvalidRead,
    #[error("Could not determine the drm version")]
    Version(#[source] OsError),
    #[error("Format of IN_FORMATS property is invalid")]
    InFormats,
    #[error("Could not import a sync obj")]
    ImportSyncObj(#[source] OsError),
    #[error("Could not create a sync obj")]
    CreateSyncObj(#[source] OsError),
    #[error("Could not export a sync obj")]
    ExportSyncObj(#[source] OsError),
    #[error("Could not register an eventfd with a sync obj")]
    RegisterEventfd(#[source] OsError),
    #[error("Could not create an eventfd")]
    EventFd(#[source] OsError),
    #[error("Could not read from an eventfd")]
    ReadEventFd(#[source] IoUringError),
    #[error("No sync obj context available")]
    NoSyncObjContextAvailable,
    #[error("Could not signal the sync obj")]
    SignalSyncObj(#[source] OsError),
    #[error("Could not transfer a sync obj point")]
    TransferPoint(#[source] OsError),
    #[error("Could not merge two sync files")]
    Merge(#[source] OsError),
    #[error("Could not import a sync file into a sync obj")]
    ImportSyncFile(#[source] OsError),
    #[error("Could not create a lease")]
    CreateLease(#[source] OsError),
    #[error("Could not drop DRM master")]
    DropMaster(#[source] OsError),
    #[error("Could not queue a CRTC sequence")]
    QueueSequence(#[source] OsError),
    #[error("Could not stat the DRM fd")]
    Stat(#[source] OsError),
}

fn render_node_name(fd: c::c_int) -> Result<Ustring, DrmError> {
    get_minor_name_from_fd(fd, NodeType::Render).map_err(DrmError::RenderNodeName)
}

fn device_node_name(fd: c::c_int) -> Result<Ustring, DrmError> {
    get_device_name_from_fd2(fd).map_err(DrmError::DeviceNodeName)
}

fn reopen(fd: c::c_int, need_primary: bool) -> Result<Rc<OwnedFd>, DrmError> {
    if let Ok((fd, _)) = create_lease(fd, &[], c::O_CLOEXEC as _) {
        return Ok(Rc::new(fd));
    }
    let path = 'path: {
        if get_node_type_from_fd(fd).map_err(DrmError::GetDeviceType)? == NodeType::Render {
            break 'path uapi::format_ustr!("/proc/self/fd/{}", fd);
        }
        if !need_primary && let Ok(path) = render_node_name(fd) {
            break 'path path;
        }
        device_node_name(fd)?
    };
    match uapi::open(path, c::O_RDWR | c::O_CLOEXEC, 0) {
        Ok(f) => Ok(Rc::new(f)),
        Err(e) => Err(DrmError::ReopenNode(e.into())),
    }
}

pub struct Drm {
    fd: Rc<OwnedFd>,
    dev: c::dev_t,
}

impl Drm {
    pub fn open_existing(fd: Rc<OwnedFd>) -> Result<Self, DrmError> {
        let stat = uapi::fstat(fd.raw()).map_err(|e| DrmError::Stat(e.into()))?;
        Ok(Self {
            fd,
            dev: stat.st_rdev,
        })
    }

    pub fn reopen(fd: c::c_int, need_primary: bool) -> Result<Self, DrmError> {
        Self::open_existing(reopen(fd, need_primary)?)
    }

    pub fn dev(&self) -> c::dev_t {
        self.dev
    }

    pub fn fd(&self) -> &Rc<OwnedFd> {
        &self.fd
    }

    pub fn raw(&self) -> c::c_int {
        self.fd.raw()
    }

    pub fn dup_render(&self) -> Result<Self, DrmError> {
        Self::reopen(self.fd.raw(), false)
    }

    pub fn get_nodes(&self) -> Result<AHashMap<NodeType, CString>, DrmError> {
        get_nodes(self.fd.raw()).map_err(DrmError::GetNodes)
    }

    pub fn get_render_node(&self) -> Result<Option<CString>, DrmError> {
        let nodes = self.get_nodes()?;
        Ok(nodes
            .get(&NodeType::Render)
            .or_else(|| nodes.get(&NodeType::Primary))
            .map(|c| c.to_owned()))
    }

    pub fn version(&self) -> Result<DrmVersion, DrmError> {
        get_version(self.fd.raw()).map_err(DrmError::Version)
    }

    pub fn drop_master(&self) -> Result<(), DrmError> {
        drop_master(self.fd.raw()).map_err(DrmError::DropMaster)
    }

    pub fn is_master(&self) -> bool {
        auth_magic(self.fd.raw(), 0) != Err(OsError(c::EACCES))
    }

    pub fn queue_sequence(&self, crtc: DrmCrtc) -> Result<(), DrmError> {
        queue_sequence(self.fd.raw(), crtc).map_err(DrmError::QueueSequence)
    }
}

pub struct InFormat {
    pub format: u32,
    pub modifiers: IndexSet<Modifier>,
}

pub struct DrmMaster {
    drm: Drm,
    u32_bufs: Stack<Vec<u32>>,
    u64_bufs: Stack<Vec<u64>>,
    gem_handles: RefCell<AHashMap<u32, Weak<GemHandle>>>,
    events: SyncQueue<DrmEvent>,
    ring: Rc<IoUring>,
    buf: RefCell<Buf>,
}

impl Debug for DrmMaster {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.drm.raw())
    }
}

impl Deref for DrmMaster {
    type Target = Drm;

    fn deref(&self) -> &Self::Target {
        &self.drm
    }
}

pub struct DrmLease {
    drm_fd: Rc<OwnedFd>,
    lessee_id: u32,
    lessee_fd: Rc<OwnedFd>,
    revoked: Cell<bool>,
}

impl DrmLease {
    pub fn lessee_fd(&self) -> &Rc<OwnedFd> {
        &self.lessee_fd
    }

    pub fn try_revoke(&self) -> bool {
        if self.revoked.get() {
            return true;
        }
        match revoke_lease(self.drm_fd.raw(), self.lessee_id) {
            Ok(_) => {
                log::info!("Revoked lease {}/{}", self.drm_fd.raw(), self.lessee_id);
                self.revoked.set(true);
                true
            }
            Err(e) => {
                log::error!("Could not revoke lease: {}", ErrorFmt(e));
                false
            }
        }
    }
}

impl DrmMaster {
    pub fn new(ring: &Rc<IoUring>, fd: Rc<OwnedFd>) -> Result<Self, DrmError> {
        Ok(Self {
            drm: Drm::open_existing(fd)?,
            u32_bufs: Default::default(),
            u64_bufs: Default::default(),
            gem_handles: Default::default(),
            events: Default::default(),
            ring: ring.clone(),
            buf: RefCell::new(Buf::new(1024)),
        })
    }

    pub fn raw(&self) -> c::c_int {
        self.drm.raw()
    }

    pub fn get_property(&self, prop: DrmProperty) -> Result<DrmPropertyDefinition, DrmError> {
        mode_getproperty(self.raw(), prop)
    }

    pub fn get_properties<T: DrmObject>(&self, t: T) -> Result<Vec<DrmPropertyValue>, DrmError> {
        mode_obj_getproperties(self.raw(), t.id(), T::TYPE)
    }

    pub fn get_resources(&self) -> Result<DrmCardResources, DrmError> {
        mode_get_resources(self.raw())
    }

    pub fn get_cap(&self, cap: u64) -> Result<u64, OsError> {
        get_cap(self.raw(), cap)
    }

    pub fn set_client_cap(&self, cap: u64, value: u64) -> Result<(), OsError> {
        set_client_cap(self.raw(), cap, value)
    }

    pub fn get_planes(&self) -> Result<Vec<DrmPlane>, DrmError> {
        mode_getplaneresources(self.raw())
    }

    pub fn get_plane_info(&self, plane: DrmPlane) -> Result<DrmPlaneInfo, DrmError> {
        mode_getplane(self.raw(), plane.0)
    }

    pub fn get_encoder_info(&self, encoder: DrmEncoder) -> Result<DrmEncoderInfo, DrmError> {
        mode_getencoder(self.raw(), encoder.0)
    }

    pub fn get_cursor_size(&self) -> Result<(u64, u64), OsError> {
        let width = self.get_cap(DRM_CAP_CURSOR_WIDTH)?;
        let height = self.get_cap(DRM_CAP_CURSOR_HEIGHT)?;
        Ok((width, height))
    }

    pub fn supports_async_commit(&self) -> bool {
        self.get_cap(DRM_CAP_ATOMIC_ASYNC_PAGE_FLIP) == Ok(1)
    }

    pub fn get_connector_info(
        &self,
        connector: DrmConnector,
        force: bool,
    ) -> Result<DrmConnectorInfo, DrmError> {
        mode_getconnector(self.raw(), connector.0, force)
    }

    pub fn change(self: &Rc<Self>) -> Change {
        let mut res = Change {
            master: self.clone(),
            objects: self.u32_bufs.pop().unwrap_or_default(),
            object_lengths: self.u32_bufs.pop().unwrap_or_default(),
            props: self.u32_bufs.pop().unwrap_or_default(),
            values: self.u64_bufs.pop().unwrap_or_default(),
        };
        res.objects.clear();
        res.object_lengths.clear();
        res.props.clear();
        res.values.clear();
        res
    }

    pub fn create_blob<T>(self: &Rc<Self>, t: &T) -> Result<PropBlob, DrmError> {
        match mode_create_blob(self.raw(), t) {
            Ok(b) => Ok(PropBlob {
                master: self.clone(),
                id: b,
            }),
            Err(e) => Err(DrmError::CreateBlob(e)),
        }
    }

    pub fn add_fb(
        self: &Rc<Self>,
        dma: &DmaBuf,
        format: Option<&Format>,
    ) -> Result<DrmFramebuffer, DrmError> {
        let mut modifier = 0;
        let mut flags = 0;
        if dma.modifier != INVALID_MODIFIER {
            modifier = dma.modifier;
            flags |= DRM_MODE_FB_MODIFIERS;
        }
        let mut strides = [0; 4];
        let mut offsets = [0; 4];
        let mut modifiers = [0; 4];
        let mut handles = [0; 4];
        let mut handles_ = vec![];
        for (idx, plane) in dma.planes.iter().enumerate() {
            strides[idx] = plane.stride;
            offsets[idx] = plane.offset;
            modifiers[idx] = modifier;
            let handle = self.gem_handle(plane.fd.raw())?;
            handles[idx] = handle.handle();
            handles_.push(handle);
        }
        match mode_addfb2(
            self.raw(),
            dma.width as _,
            dma.height as _,
            format.unwrap_or(dma.format).drm,
            flags,
            handles,
            strides,
            offsets,
            modifiers,
        ) {
            Ok(fb) => Ok(DrmFramebuffer {
                master: self.clone(),
                fb,
            }),
            Err(e) => Err(DrmError::AddFb(e)),
        }
    }

    pub fn gem_handle(self: &Rc<Self>, fd: c::c_int) -> Result<Rc<GemHandle>, DrmError> {
        let handle = match prime_fd_to_handle(self.raw(), fd) {
            Ok(h) => h,
            Err(e) => return Err(DrmError::GemHandle(e)),
        };
        let mut handles = self.gem_handles.borrow_mut();
        if let Some(h) = handles.get(&handle)
            && let Some(h) = h.upgrade()
        {
            return Ok(h);
        }
        let h = Rc::new(GemHandle {
            master: self.clone(),
            handle,
        });
        handles.insert(handle, Rc::downgrade(&h));
        Ok(h)
    }

    pub fn getblob<T: Pod>(&self, blob: DrmBlob) -> Result<T, DrmError> {
        let mut t = MaybeUninit::<T>::uninit();
        match mode_getprobblob(self.raw(), blob.0, &mut t) {
            Err(e) => Err(DrmError::GetPropBlob(e)),
            Ok(n) if n != size_of::<T>() => Err(DrmError::InvalidProbSize),
            _ => unsafe { Ok(t.assume_init()) },
        }
    }

    pub fn getblob_vec<T: Pod>(&self, blob: DrmBlob) -> Result<Vec<T>, DrmError> {
        assert_ne!(size_of::<T>(), 0);
        let mut vec = vec![];
        loop {
            let (_, bytes) = vec.split_at_spare_mut_bytes_ext();
            match mode_getprobblob(self.raw(), blob.0, bytes) {
                Err(e) => return Err(DrmError::GetPropBlob(e)),
                Ok(n) if n % size_of::<T>() != 0 => return Err(DrmError::UnalignedPropSize),
                Ok(n) if n <= bytes.len() => {
                    unsafe {
                        vec.set_len(n / size_of::<T>());
                    }
                    return Ok(vec);
                }
                Ok(n) => vec.reserve_exact(n / size_of::<T>()),
            }
        }
    }

    pub fn get_in_formats(&self, property: u32) -> Result<Vec<InFormat>, DrmError> {
        let blob = self.getblob_vec::<u8>(DrmBlob(property))?;
        let header: drm_format_modifier_blob = match uapi::pod_read_init(blob.as_bytes()) {
            Ok(h) => h,
            Err(_) => {
                log::error!("Header of IN_FORMATS blob doesn't fit in the blob");
                return Err(DrmError::InFormats);
            }
        };
        if header.version != FORMAT_BLOB_CURRENT {
            log::error!(
                "Header of IN_FORMATS has an invalid version: {}",
                header.version
            );
            return Err(DrmError::InFormats);
        }
        let formats_start = header.formats_offset as usize;
        let formats_end = formats_start
            .wrapping_add((header.count_formats as usize).wrapping_mul(size_of::<u32>()));
        let modifiers_start = header.modifiers_offset as usize;
        let modifiers_end = modifiers_start.wrapping_add(
            (header.count_modifiers as usize).wrapping_mul(size_of::<drm_format_modifier>()),
        );
        if blob.len() < formats_end || formats_end < formats_start {
            log::error!("Formats of IN_FORMATS blob don't fit in the blob");
            return Err(DrmError::InFormats);
        }
        if blob.len() < modifiers_end || modifiers_end < modifiers_start {
            log::error!("Formats of IN_FORMATS blob don't fit in the blob");
            return Err(DrmError::InFormats);
        }
        let mut formats: Vec<_> = uapi::pod_iter::<u32, _>(&blob[formats_start..formats_end])
            .unwrap()
            .map(|f| InFormat {
                format: f,
                modifiers: IndexSet::new(),
            })
            .collect();
        let modifiers =
            uapi::pod_iter::<drm_format_modifier, _>(&blob[modifiers_start..modifiers_end])
                .unwrap();
        for modifier in modifiers {
            let offset = modifier.offset as usize;
            let mut indices = modifier.formats;
            while indices != 0 {
                let idx = indices.trailing_zeros();
                indices &= !(1 << idx);
                let idx = idx as usize + offset;
                if idx >= formats.len() {
                    log::error!("Modifier offset is out of bounds");
                    return Err(DrmError::InFormats);
                }
                formats[idx].modifiers.insert(modifier.modifier);
            }
        }
        Ok(formats)
    }

    #[expect(clippy::await_holding_refcell_ref)]
    pub async fn event(&self) -> Result<Option<DrmEvent>, DrmError> {
        if self.events.is_empty() {
            let mut buf = self.buf.borrow_mut();
            let mut buf = match self.ring.read(self.drm.fd(), buf.clone()).await {
                Ok(n) => &buf[..n],
                Err(e) => return Err(DrmError::ReadEvents(e)),
            };
            while buf.len() > 0 {
                let header: drm_event = match uapi::pod_read_init(buf) {
                    Ok(e) => e,
                    _ => return Err(DrmError::InvalidRead),
                };
                let len = header.length as usize;
                if len > buf.len() {
                    return Err(DrmError::InvalidRead);
                }
                match header.ty {
                    sys::DRM_EVENT_FLIP_COMPLETE => {
                        let event: drm_event_vblank = match uapi::pod_read_init(buf) {
                            Ok(e) => e,
                            _ => return Err(DrmError::InvalidRead),
                        };
                        self.events.push(DrmEvent::FlipComplete {
                            tv_sec: event.tv_sec,
                            tv_usec: event.tv_usec,
                            sequence: event.sequence,
                            crtc_id: DrmCrtc(event.crtc_id),
                        });
                    }
                    sys::DRM_EVENT_CRTC_SEQUENCE => {
                        let event: drm_event_crtc_sequence = match uapi::pod_read_init(buf) {
                            Ok(e) => e,
                            _ => return Err(DrmError::InvalidRead),
                        };
                        self.events.push(DrmEvent::Sequence {
                            time_ns: event.time_ns,
                            sequence: event.sequence,
                            crtc_id: DrmCrtc(event.user_data as _),
                        });
                    }
                    _ => {}
                }
                buf = &buf[len..];
            }
        }
        Ok(self.events.pop())
    }

    pub fn lease(&self, objs: &[u32]) -> Result<DrmLease, DrmError> {
        let (fd, lessee_id) =
            create_lease(self.raw(), objs, c::O_CLOEXEC as _).map_err(DrmError::CreateLease)?;
        log::info!("Created lease {}/{}", self.fd.raw(), lessee_id);
        Ok(DrmLease {
            drm_fd: self.fd.clone(),
            lessee_id,
            lessee_fd: Rc::new(fd),
            revoked: Cell::new(false),
        })
    }
}

pub enum DrmEvent {
    FlipComplete {
        tv_sec: u32,
        tv_usec: u32,
        sequence: u32,
        crtc_id: DrmCrtc,
    },
    Sequence {
        time_ns: i64,
        sequence: u64,
        crtc_id: DrmCrtc,
    },
}

pub struct DrmFramebuffer {
    master: Rc<DrmMaster>,
    fb: DrmFb,
}

impl Debug for DrmFramebuffer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DrmFramebuffer")
            .field("fb", &self.fb)
            .finish_non_exhaustive()
    }
}

impl DrmFramebuffer {
    pub fn id(&self) -> DrmFb {
        self.fb
    }
}

impl Drop for DrmFramebuffer {
    fn drop(&mut self) {
        if let Err(e) = mode_rmfb(self.master.raw(), self.fb) {
            log::error!("Could not delete framebuffer: {}", ErrorFmt(e));
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum NodeType {
    Primary,
    Control,
    Render,
}

impl NodeType {
    fn name(self) -> &'static str {
        match self {
            NodeType::Primary => "card",
            NodeType::Control => "controlD",
            NodeType::Render => "renderD",
        }
    }
}

#[derive(Debug)]
pub struct DrmPropertyDefinition {
    pub id: DrmProperty,
    pub name: BString,
    pub _immutable: bool,
    pub _atomic: bool,
    pub ty: DrmPropertyType,
}

#[derive(Debug, Clone)]
pub enum DrmPropertyType {
    Range {
        _min: u64,
        max: u64,
    },
    SignedRange {
        _min: i64,
        max: i64,
    },
    Object {
        _ty: u32,
    },
    Blob,
    Enum {
        values: Vec<DrmPropertyEnumValue>,
        bitmask: bool,
    },
}

#[derive(Debug, Clone)]
pub struct DrmPropertyEnumValue {
    pub value: u64,
    pub name: BString,
}

#[derive(Debug)]
pub struct DrmPropertyValue {
    pub id: DrmProperty,
    pub value: u64,
}

pub trait DrmObject {
    const TYPE: u32;
    const NONE: Self;
    fn id(&self) -> u32;
    fn is_some(&self) -> bool;
    fn is_none(&self) -> bool;
}

macro_rules! drm_obj {
    ($name:ident, $ty:expr) => {
        #[repr(transparent)]
        #[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Default)]
        pub struct $name(pub u32);

        impl DrmObject for $name {
            const TYPE: u32 = $ty;
            const NONE: Self = Self(0);

            fn id(&self) -> u32 {
                self.0
            }

            fn is_some(&self) -> bool {
                self.0 != 0
            }

            fn is_none(&self) -> bool {
                self.0 == 0
            }
        }
    };
}
drm_obj!(DrmCrtc, DRM_MODE_OBJECT_CRTC);
drm_obj!(DrmConnector, DRM_MODE_OBJECT_CONNECTOR);
drm_obj!(DrmEncoder, DRM_MODE_OBJECT_ENCODER);
drm_obj!(DrmMode, DRM_MODE_OBJECT_MODE);
drm_obj!(DrmProperty, DRM_MODE_OBJECT_PROPERTY);
drm_obj!(DrmFb, DRM_MODE_OBJECT_FB);
drm_obj!(DrmBlob, DRM_MODE_OBJECT_BLOB);
drm_obj!(DrmPlane, DRM_MODE_OBJECT_PLANE);

#[derive(Debug)]
pub struct DrmCardResources {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
    pub _fbs: Vec<DrmFb>,
    pub crtcs: Vec<DrmCrtc>,
    pub connectors: Vec<DrmConnector>,
    pub encoders: Vec<DrmEncoder>,
}

#[derive(Debug)]
pub struct DrmPlaneInfo {
    pub _plane_id: DrmPlane,
    pub _crtc_id: DrmCrtc,
    pub _fb_id: DrmFb,
    pub possible_crtcs: u32,
    pub _gamma_size: u32,
    pub format_types: Vec<u32>,
}

#[derive(Debug)]
pub struct DrmEncoderInfo {
    pub _encoder_id: DrmEncoder,
    pub _encoder_type: u32,
    pub _crtc_id: DrmCrtc,
    pub possible_crtcs: u32,
    pub _possible_clones: u32,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DrmModeInfo {
    pub clock: u32,
    pub hdisplay: u16,
    pub hsync_start: u16,
    pub hsync_end: u16,
    pub htotal: u16,
    pub hskew: u16,
    pub vdisplay: u16,
    pub vsync_start: u16,
    pub vsync_end: u16,
    pub vtotal: u16,
    pub vscan: u16,

    pub vrefresh: u32,

    pub flags: u32,
    pub ty: u32,
    pub name: BString,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DrmVersion {
    pub version_major: i32,
    pub version_minor: i32,
    pub version_patchlevel: i32,
    pub name: BString,
    pub date: BString,
    pub desc: BString,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct HdrMetadata {
    pub eotf: u8,
    pub metadata_type: u8,
    pub red: (u16, u16),
    pub green: (u16, u16),
    pub blue: (u16, u16),
    pub white: (u16, u16),
    pub max_display_mastering_luminance: u16,
    pub min_display_mastering_luminance: u16,
    pub max_cll: u16,
    pub max_fall: u16,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub struct hdr_metadata_primary {
    pub x: u16,
    pub y: u16,
}

#[repr(C)]
#[derive(Copy, Clone)]
union hdr_output_metadata_type {
    hdmi_metadata_type1: hdr_metadata_infoframe,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct hdr_output_metadata {
    metadata_type: u32,
    ty: hdr_output_metadata_type,
}

impl hdr_output_metadata {
    pub fn new(infoframe: hdr_metadata_infoframe) -> Self {
        Self {
            metadata_type: 0,
            ty: hdr_output_metadata_type {
                hdmi_metadata_type1: infoframe,
            },
        }
    }

    pub fn from_eotf(eotf: u8) -> Self {
        Self::new(hdr_metadata_infoframe {
            eotf,
            metadata_type: 0,
            ..hdr_metadata_infoframe::default()
        })
    }
}

unsafe impl Pod for hdr_output_metadata {}

impl Debug for hdr_output_metadata {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut f = f.debug_struct("hdr_output_metadata");
        f.field("metadata_type", &self.metadata_type);
        match self.metadata_type {
            0 => unsafe {
                f.field("hdmi_metadata_type1", &self.ty.hdmi_metadata_type1)
                    .finish()
            },
            _ => f.finish_non_exhaustive(),
        }
    }
}

impl Eq for hdr_output_metadata {}

impl PartialEq for hdr_output_metadata {
    fn eq(&self, other: &Self) -> bool {
        if self.metadata_type != other.metadata_type {
            return false;
        }
        match self.metadata_type {
            0 => unsafe {
                self.ty
                    .hdmi_metadata_type1
                    .eq(&other.ty.hdmi_metadata_type1)
            },
            _ => return false,
        }
    }
}

#[expect(dead_code)]
mod consts {
    pub const HDMI_EOTF_TRADITIONAL_GAMMA_SDR: u8 = 0;
    pub const HDMI_EOTF_TRADITIONAL_GAMMA_HDR: u8 = 1;
    pub const HDMI_EOTF_SMPTE_ST2084: u8 = 2;
    pub const HDMI_EOTF_BT_2100_HLG: u8 = 3;

    pub const DRM_MODE_COLORIMETRY_DEFAULT: u64 = 0;
    pub const DRM_MODE_COLORIMETRY_NO_DATA: u64 = 0;
    pub const DRM_MODE_COLORIMETRY_SMPTE_170M_YCC: u64 = 1;
    pub const DRM_MODE_COLORIMETRY_BT709_YCC: u64 = 2;
    pub const DRM_MODE_COLORIMETRY_XVYCC_601: u64 = 3;
    pub const DRM_MODE_COLORIMETRY_XVYCC_709: u64 = 4;
    pub const DRM_MODE_COLORIMETRY_SYCC_601: u64 = 5;
    pub const DRM_MODE_COLORIMETRY_OPYCC_601: u64 = 6;
    pub const DRM_MODE_COLORIMETRY_OPRGB: u64 = 7;
    pub const DRM_MODE_COLORIMETRY_BT2020_CYCC: u64 = 8;
    pub const DRM_MODE_COLORIMETRY_BT2020_RGB: u64 = 9;
    pub const DRM_MODE_COLORIMETRY_BT2020_YCC: u64 = 10;
    pub const DRM_MODE_COLORIMETRY_DCI_P3_RGB_D65: u64 = 11;
    pub const DRM_MODE_COLORIMETRY_DCI_P3_RGB_THEATER: u64 = 12;
    pub const DRM_MODE_COLORIMETRY_RGB_WIDE_FIXED: u64 = 13;
    pub const DRM_MODE_COLORIMETRY_RGB_WIDE_FLOAT: u64 = 14;
    pub const DRM_MODE_COLORIMETRY_BT601_YCC: u64 = 15;
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub struct hdr_metadata_infoframe {
    pub eotf: u8,
    pub metadata_type: u8,
    pub display_primaries: [hdr_metadata_primary; 3],
    pub white_point: hdr_metadata_primary,
    pub max_display_mastering_luminance: u16,
    pub min_display_mastering_luminance: u16,
    pub max_cll: u16,
    pub max_fall: u16,
}

impl DrmModeInfo {
    pub fn create_blob(&self, master: &Rc<DrmMaster>) -> Result<PropBlob, DrmError> {
        let raw = self.to_raw();
        master.create_blob(&raw)
    }

    pub fn to_raw(&self) -> drm_mode_modeinfo {
        let mut name = [0u8; DRM_DISPLAY_MODE_LEN];
        let len = name.len().min(self.name.len());
        name[..len].copy_from_slice(&self.name.as_bytes()[..len]);
        drm_mode_modeinfo {
            clock: self.clock,
            hdisplay: self.hdisplay,
            hsync_start: self.hsync_start,
            hsync_end: self.hsync_end,
            htotal: self.htotal,
            hskew: self.hskew,
            vdisplay: self.vdisplay,
            vsync_start: self.vsync_start,
            vsync_end: self.vsync_end,
            vtotal: self.vtotal,
            vscan: self.vscan,
            vrefresh: self.vrefresh,
            flags: self.flags,
            ty: self.ty,
            name,
        }
    }

    pub fn to_backend(&self) -> backend::Mode {
        backend::Mode {
            width: self.hdisplay as _,
            height: self.vdisplay as _,
            refresh_rate_millihz: self.refresh_rate_millihz(),
        }
    }

    pub fn refresh_rate_millihz(&self) -> u32 {
        let clock_millihz = self.clock as u64 * 1_000_000;
        let htotal = self.htotal as u64;
        let vtotal = self.vtotal as u64;
        (((clock_millihz / htotal) + (vtotal / 2)) / vtotal) as u32
        // simplifies to
        //     clock_millihz / (htotal * vtotal) + 1/2
        // why round up (+1/2) instead of down?
    }
}

#[derive(Debug)]
pub struct DrmConnectorInfo {
    pub encoders: Vec<DrmEncoder>,
    pub modes: Vec<DrmModeInfo>,
    pub _props: Vec<DrmPropertyValue>,

    pub _encoder_id: DrmEncoder,
    pub _connector_id: DrmConnector,
    pub connector_type: u32,
    pub connector_type_id: u32,

    pub connection: u32,
    pub mm_width: u32,
    pub mm_height: u32,
    pub subpixel: u32,
}

pub struct Change {
    master: Rc<DrmMaster>,
    objects: Vec<u32>,
    object_lengths: Vec<u32>,
    props: Vec<u32>,
    values: Vec<u64>,
}

pub struct ObjectChange<'a> {
    change: &'a mut Change,
}

impl Change {
    #[expect(dead_code)]
    pub fn test(&self, flags: u32) -> Result<(), DrmError> {
        mode_atomic(
            self.master.raw(),
            flags | DRM_MODE_ATOMIC_TEST_ONLY,
            &self.objects,
            &self.object_lengths,
            &self.props,
            &self.values,
            0,
        )
    }

    pub fn commit(&self, flags: u32, user_data: u64) -> Result<(), DrmError> {
        mode_atomic(
            self.master.raw(),
            flags,
            &self.objects,
            &self.object_lengths,
            &self.props,
            &self.values,
            user_data,
        )
    }

    pub fn change_object<T, F>(&mut self, obj: T, f: F)
    where
        T: DrmObject,
        F: FnOnce(&mut ObjectChange),
    {
        let old_len = self.props.len();
        let mut oc = ObjectChange { change: self };
        f(&mut oc);
        if self.props.len() > old_len {
            let new = (self.props.len() - old_len) as u32;
            if self.objects.last() == Some(&obj.id()) {
                *self.object_lengths.last_mut().unwrap() += new;
            } else {
                self.objects.push(obj.id());
                self.object_lengths.push(new);
            }
        }
    }
}

impl<'a> ObjectChange<'a> {
    pub fn change(&mut self, property_id: DrmProperty, value: u64) {
        self.change.props.push(property_id.0);
        self.change.values.push(value);
    }
}

impl Drop for Change {
    fn drop(&mut self) {
        self.master.u32_bufs.push(mem::take(&mut self.objects));
        self.master
            .u32_bufs
            .push(mem::take(&mut self.object_lengths));
        self.master.u32_bufs.push(mem::take(&mut self.props));
        self.master.u64_bufs.push(mem::take(&mut self.values));
    }
}

#[expect(non_camel_case_types)]
#[derive(Copy, Clone, Debug)]
pub enum ConnectorType {
    Unknown(u32),
    VGA,
    DVII,
    DVID,
    DVIA,
    Composite,
    SVIDEO,
    LVDS,
    Component,
    _9PinDIN,
    DisplayPort,
    HDMIA,
    HDMIB,
    TV,
    eDP,
    VIRTUAL,
    DSI,
    DPI,
    WRITEBACK,
    SPI,
    USB,
    EmbeddedWindow,
}

impl ConnectorType {
    pub fn from_drm(v: u32) -> Self {
        match v {
            sys::DRM_MODE_CONNECTOR_VGA => Self::VGA,
            sys::DRM_MODE_CONNECTOR_DVII => Self::DVII,
            sys::DRM_MODE_CONNECTOR_DVID => Self::DVID,
            sys::DRM_MODE_CONNECTOR_DVIA => Self::DVIA,
            sys::DRM_MODE_CONNECTOR_Composite => Self::Composite,
            sys::DRM_MODE_CONNECTOR_SVIDEO => Self::SVIDEO,
            sys::DRM_MODE_CONNECTOR_LVDS => Self::LVDS,
            sys::DRM_MODE_CONNECTOR_Component => Self::Component,
            sys::DRM_MODE_CONNECTOR_9PinDIN => Self::_9PinDIN,
            sys::DRM_MODE_CONNECTOR_DisplayPort => Self::DisplayPort,
            sys::DRM_MODE_CONNECTOR_HDMIA => Self::HDMIA,
            sys::DRM_MODE_CONNECTOR_HDMIB => Self::HDMIB,
            sys::DRM_MODE_CONNECTOR_TV => Self::TV,
            sys::DRM_MODE_CONNECTOR_eDP => Self::eDP,
            sys::DRM_MODE_CONNECTOR_VIRTUAL => Self::VIRTUAL,
            sys::DRM_MODE_CONNECTOR_DSI => Self::DSI,
            sys::DRM_MODE_CONNECTOR_DPI => Self::DPI,
            sys::DRM_MODE_CONNECTOR_WRITEBACK => Self::WRITEBACK,
            sys::DRM_MODE_CONNECTOR_SPI => Self::SPI,
            sys::DRM_MODE_CONNECTOR_USB => Self::USB,
            _ => Self::Unknown(v),
        }
    }

    #[expect(dead_code)]
    pub fn to_drm(self) -> u32 {
        match self {
            Self::Unknown(n) => n,
            Self::VGA => sys::DRM_MODE_CONNECTOR_VGA,
            Self::DVII => sys::DRM_MODE_CONNECTOR_DVII,
            Self::DVID => sys::DRM_MODE_CONNECTOR_DVID,
            Self::DVIA => sys::DRM_MODE_CONNECTOR_DVIA,
            Self::Composite => sys::DRM_MODE_CONNECTOR_Composite,
            Self::SVIDEO => sys::DRM_MODE_CONNECTOR_SVIDEO,
            Self::LVDS => sys::DRM_MODE_CONNECTOR_LVDS,
            Self::Component => sys::DRM_MODE_CONNECTOR_Component,
            Self::_9PinDIN => sys::DRM_MODE_CONNECTOR_9PinDIN,
            Self::DisplayPort => sys::DRM_MODE_CONNECTOR_DisplayPort,
            Self::HDMIA => sys::DRM_MODE_CONNECTOR_HDMIA,
            Self::HDMIB => sys::DRM_MODE_CONNECTOR_HDMIB,
            Self::TV => sys::DRM_MODE_CONNECTOR_TV,
            Self::eDP => sys::DRM_MODE_CONNECTOR_eDP,
            Self::VIRTUAL => sys::DRM_MODE_CONNECTOR_VIRTUAL,
            Self::DSI => sys::DRM_MODE_CONNECTOR_DSI,
            Self::DPI => sys::DRM_MODE_CONNECTOR_DPI,
            Self::WRITEBACK => sys::DRM_MODE_CONNECTOR_WRITEBACK,
            Self::SPI => sys::DRM_MODE_CONNECTOR_SPI,
            Self::USB => sys::DRM_MODE_CONNECTOR_USB,
            Self::EmbeddedWindow => sys::DRM_MODE_CONNECTOR_Unknown,
        }
    }

    pub fn to_config(self) -> jay_config::video::connector_type::ConnectorType {
        use jay_config::video::connector_type::*;
        match self {
            Self::Unknown(_) => CON_UNKNOWN,
            Self::VGA => CON_VGA,
            Self::DVII => CON_DVII,
            Self::DVID => CON_DVID,
            Self::DVIA => CON_DVIA,
            Self::Composite => CON_COMPOSITE,
            Self::SVIDEO => CON_SVIDEO,
            Self::LVDS => CON_LVDS,
            Self::Component => CON_COMPONENT,
            Self::_9PinDIN => CON_9PIN_DIN,
            Self::DisplayPort => CON_DISPLAY_PORT,
            Self::HDMIA => CON_HDMIA,
            Self::HDMIB => CON_HDMIB,
            Self::TV => CON_TV,
            Self::eDP => CON_EDP,
            Self::VIRTUAL => CON_VIRTUAL,
            Self::DSI => CON_DSI,
            Self::DPI => CON_DPI,
            Self::WRITEBACK => CON_WRITEBACK,
            Self::SPI => CON_SPI,
            Self::USB => CON_USB,
            Self::EmbeddedWindow => CON_EMBEDDED_WINDOW,
        }
    }
}

impl Display for ConnectorType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Unknown(n) => return write!(f, "Unknown({})", n),
            Self::VGA => "VGA",
            Self::DVII => "DVI-I",
            Self::DVID => "DVI-D",
            Self::DVIA => "DVI-A",
            Self::Composite => "Composite",
            Self::SVIDEO => "SVIDEO",
            Self::LVDS => "LVDS",
            Self::Component => "Component",
            Self::_9PinDIN => "DIN",
            Self::DisplayPort => "DP",
            Self::HDMIA => "HDMI-A",
            Self::HDMIB => "HDMI-B",
            Self::TV => "TV",
            Self::eDP => "eDP",
            Self::VIRTUAL => "Virtual",
            Self::DSI => "DSI",
            Self::DPI => "DPI",
            Self::WRITEBACK => "Writeback",
            Self::SPI => "SPI",
            Self::USB => "USB",
            Self::EmbeddedWindow => "EmbeddedWindow",
        };
        f.write_str(s)
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ConnectorStatus {
    Connected,
    Disconnected,
    Unknown,
    Other(u32),
}

impl ConnectorStatus {
    pub fn from_drm(v: u32) -> Self {
        match v {
            sys::CONNECTOR_STATUS_CONNECTED => Self::Connected,
            sys::CONNECTOR_STATUS_DISCONNECTED => Self::Disconnected,
            sys::CONNECTOR_STATUS_UNKNOWN => Self::Unknown,
            _ => Self::Other(v),
        }
    }
}

#[derive(Debug)]
pub struct PropBlob {
    master: Rc<DrmMaster>,
    id: DrmBlob,
}

impl PropBlob {
    pub fn id(&self) -> DrmBlob {
        self.id
    }
}

impl Drop for PropBlob {
    fn drop(&mut self) {
        if let Err(e) = mode_destroy_blob(self.master.raw(), self.id) {
            log::error!("Could not destroy blob: {}", ErrorFmt(e));
        }
    }
}

pub struct GemHandle {
    master: Rc<DrmMaster>,
    handle: u32,
}

impl GemHandle {
    pub fn handle(&self) -> u32 {
        self.handle
    }
}

impl Drop for GemHandle {
    fn drop(&mut self) {
        self.master.gem_handles.borrow_mut().remove(&self.handle);
        if let Err(e) = gem_close(self.master.raw(), self.handle) {
            log::error!("Could not close gem handle: {}", ErrorFmt(e));
        }
    }
}
