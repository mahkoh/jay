#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]

use {
    crate::{
        utils::{bitflags::BitflagsExt, compat::IoctlNumber, oserror::OsError},
        video::drm::{
            DrmBlob, DrmCardResources, DrmConnector, DrmConnectorInfo, DrmCrtc, DrmEncoder,
            DrmEncoderInfo, DrmError, DrmFb, DrmModeInfo, DrmPlane, DrmPlaneInfo, DrmProperty,
            DrmPropertyDefinition, DrmPropertyEnumValue, DrmPropertyType, DrmPropertyValue,
            DrmVersion, NodeType,
        },
    },
    ahash::AHashMap,
    bstr::ByteSlice,
    std::{
        ffi::CString,
        io::{BufRead, BufReader},
    },
    uapi::{OwnedFd, Pod, Ustring, c, pod_zeroed},
};

pub unsafe fn ioctl<T>(fd: c::c_int, request: c::c_ulong, t: &mut T) -> Result<c::c_int, OsError> {
    let mut ret;
    loop {
        ret = unsafe { c::ioctl(fd, request as IoctlNumber, &mut *t) };
        if ret != -1 {
            return Ok(ret);
        }
        let err = uapi::get_errno();
        if !matches!(err, c::EINTR | c::EAGAIN) {
            return Err(OsError(err));
        }
    }
}

pub const DRM_IOCTL_BASE: u64 = b'd' as u64;

pub const fn drm_io(nr: u64) -> u64 {
    uapi::_IO(DRM_IOCTL_BASE, nr)
}

pub const fn drm_iow<T>(nr: u64) -> u64 {
    uapi::_IOW::<T>(DRM_IOCTL_BASE, nr)
}

pub const fn drm_iowr<T>(nr: u64) -> u64 {
    uapi::_IOWR::<T>(DRM_IOCTL_BASE, nr)
}

const DRM_IOCTL_MODE_CREATE_LEASE: u64 = drm_iowr::<drm_mode_create_lease>(0xc6);

#[repr(C)]
struct drm_mode_create_lease {
    object_ids: u64,
    object_count: u32,
    flags: u32,
    lessee_id: u32,
    fd: u32,
}

pub fn create_lease(fd: c::c_int, objects: &[u32], flags: u32) -> Result<(OwnedFd, u32), OsError> {
    let mut create = drm_mode_create_lease {
        object_ids: objects.as_ptr() as usize as _,
        object_count: objects.len() as _,
        flags,
        lessee_id: 0,
        fd: 0,
    };
    unsafe {
        ioctl(fd, DRM_IOCTL_MODE_CREATE_LEASE, &mut create)?;
    }
    Ok((OwnedFd::new(create.fd as _), create.lessee_id))
}

const DRM_IOCTL_MODE_REVOKE_LEASE: u64 = drm_iowr::<drm_mode_revoke_lease>(0xc9);

#[repr(C)]
struct drm_mode_revoke_lease {
    lessee_id: u32,
}

pub fn revoke_lease(fd: c::c_int, lessee_id: u32) -> Result<(), OsError> {
    let mut revoke = drm_mode_revoke_lease { lessee_id };
    unsafe {
        ioctl(fd, DRM_IOCTL_MODE_REVOKE_LEASE, &mut revoke)?;
    }
    Ok(())
}

pub fn get_node_type_from_fd(fd: c::c_int) -> Result<NodeType, OsError> {
    let (_, _, min) = drm_stat(fd)?;
    get_minor_type(min)
}

pub fn node_is_drm(maj: u64, min: u64) -> bool {
    let path = device_dir(maj, min);
    uapi::stat(path).is_ok()
}

pub fn get_minor_type(min: u64) -> Result<NodeType, OsError> {
    const DRM_NODE_PRIMARY: u64 = 0;
    const DRM_NODE_CONTROL: u64 = 1;
    const DRM_NODE_RENDER: u64 = 2;
    match min >> 6 {
        DRM_NODE_PRIMARY => Ok(NodeType::Primary),
        DRM_NODE_CONTROL => Ok(NodeType::Control),
        DRM_NODE_RENDER => Ok(NodeType::Render),
        _ => Err(OsError(c::ENODEV)),
    }
}

const DRM_DIR_NAME: &str = "/dev/dri";

fn device_dir(maj: u64, min: u64) -> Ustring {
    uapi::format_ustr!("/sys/dev/char/{maj}:{min}/device/drm")
}

pub fn get_minor_name_from_fd(fd: c::c_int, ty: NodeType) -> Result<Ustring, OsError> {
    let (_, maj, min) = drm_stat(fd)?;

    let dir = device_dir(maj, min);
    let mut dir = uapi::opendir(dir)?;

    while let Some(entry) = uapi::readdir(&mut dir) {
        let entry = entry?;
        if entry.name().to_bytes().starts_with_str(ty.name()) {
            return Ok(uapi::format_ustr!(
                "{}/{}",
                DRM_DIR_NAME,
                entry.name().to_bytes().as_bstr()
            ));
        }
    }
    Err(OsError(c::ENOENT))
}

fn drm_stat(fd: c::c_int) -> Result<(c::stat, u64, u64), OsError> {
    let stat = uapi::fstat(fd)?;

    let maj = uapi::major(stat.st_rdev);
    let min = uapi::minor(stat.st_rdev);

    if !is_drm(maj, min, &stat) {
        return Err(OsError(c::ENODEV));
    }

    Ok((stat, maj, min))
}

fn is_drm(maj: u64, min: u64, stat: &c::stat) -> bool {
    stat.st_mode & c::S_IFMT == c::S_IFCHR && node_is_drm(maj, min)
}

pub fn get_device_name_from_fd2(fd: c::c_int) -> Result<Ustring, OsError> {
    let (_, maj, min) = drm_stat(fd)?;
    let path = uapi::format_ustr!("/sys/dev/char/{maj}:{min}/uevent");
    let mut buf = vec![];
    let mut br = BufReader::new(uapi::open(path, c::O_RDONLY, 0)?);
    loop {
        buf.clear();
        if br.read_until(b'\n', &mut buf)? == 0 {
            break;
        }
        if let Some(pf) = buf.strip_prefix(b"DEVNAME=") {
            return Ok(uapi::format_ustr!("/dev/{}", pf.trim_ascii_end().as_bstr()));
        }
    }
    Err(OsError(c::ENOENT))
}

pub fn get_nodes(fd: c::c_int) -> Result<AHashMap<NodeType, CString>, OsError> {
    let (_, maj, min) = drm_stat(fd)?;

    let dir = device_dir(maj, min);
    let mut dir = uapi::opendir(dir)?;

    let mut res = AHashMap::new();

    'outer: while let Some(entry) = uapi::readdir(&mut dir) {
        let entry = entry?;
        let name = entry.name().to_bytes();
        let ty = 'ty: {
            for ty in [NodeType::Render, NodeType::Control, NodeType::Primary] {
                if name.starts_with_str(ty.name()) {
                    break 'ty ty;
                }
            }
            continue 'outer;
        };
        res.insert(
            ty,
            uapi::format_ustr!("{}/{}", DRM_DIR_NAME, name.as_bstr())
                .into_c_string()
                .unwrap(),
        );
    }

    Ok(res)
}

const DRM_PROP_NAME_LEN: usize = 32;

#[repr(C)]
#[derive(Default)]
struct drm_mode_get_property {
    values_ptr: u64,
    enum_blob_ptr: u64,
    prop_id: u32,
    flags: u32,
    name: [u8; DRM_PROP_NAME_LEN],
    count_values: u32,
    count_enum_blobs: u32,
}

const DRM_IOCTL_MODE_GETPROPERTY: u64 = drm_iowr::<drm_mode_get_property>(0xaa);

#[expect(dead_code)]
const DRM_MODE_PROP_PENDING: u32 = 1 << 0;
const DRM_MODE_PROP_RANGE: u32 = 1 << 1;
const DRM_MODE_PROP_IMMUTABLE: u32 = 1 << 2;
const DRM_MODE_PROP_ENUM: u32 = 1 << 3;
const DRM_MODE_PROP_BLOB: u32 = 1 << 4;
const DRM_MODE_PROP_BITMASK: u32 = 1 << 5;

const DRM_MODE_PROP_LEGACY_TYPE: u32 =
    DRM_MODE_PROP_RANGE | DRM_MODE_PROP_ENUM | DRM_MODE_PROP_BLOB | DRM_MODE_PROP_BITMASK;

const DRM_MODE_PROP_EXTENDED_TYPE: u32 = 0x0000ffc0;
const fn drm_mode_prop_type(n: u32) -> u32 {
    n << 6
}
const DRM_MODE_PROP_OBJECT: u32 = drm_mode_prop_type(1);
const DRM_MODE_PROP_SIGNED_RANGE: u32 = drm_mode_prop_type(2);

const DRM_MODE_PROP_ATOMIC: u32 = 0x80000000;

pub const DRM_CAP_CURSOR_WIDTH: u64 = 0x8;
pub const DRM_CAP_CURSOR_HEIGHT: u64 = 0x9;
pub const DRM_CAP_ATOMIC_ASYNC_PAGE_FLIP: u64 = 0x15;

#[repr(C)]
struct drm_mode_property_enum {
    value: u64,
    name: [u8; DRM_PROP_NAME_LEN],
}

pub fn mode_getproperty(
    fd: c::c_int,
    property_id: DrmProperty,
) -> Result<DrmPropertyDefinition, DrmError> {
    let mut prop = drm_mode_get_property {
        prop_id: property_id.0,
        ..Default::default()
    };

    let get = |prop: &mut drm_mode_get_property| {
        unsafe {
            if let Err(e) = ioctl(fd, DRM_IOCTL_MODE_GETPROPERTY, prop) {
                return Err(DrmError::GetProperty(e));
            }
        }
        Ok(())
    };

    get(&mut prop)?;

    let ty = prop.flags & (DRM_MODE_PROP_LEGACY_TYPE | DRM_MODE_PROP_EXTENDED_TYPE);
    let ty = match ty {
        DRM_MODE_PROP_RANGE | DRM_MODE_PROP_SIGNED_RANGE => {
            if prop.count_values != 2 {
                return Err(DrmError::RangeValues);
            }
            prop.count_enum_blobs = 0;
            let mut vals = [0u64, 0];
            prop.values_ptr = vals.as_mut_ptr() as _;
            get(&mut prop)?;
            if ty == DRM_MODE_PROP_RANGE {
                DrmPropertyType::Range {
                    _min: vals[0],
                    max: vals[1],
                }
            } else {
                DrmPropertyType::SignedRange {
                    _min: vals[0] as _,
                    max: vals[1] as _,
                }
            }
        }
        DRM_MODE_PROP_ENUM | DRM_MODE_PROP_BITMASK => {
            prop.count_values = 0;
            let mut props =
                Vec::<drm_mode_property_enum>::with_capacity(prop.count_enum_blobs as usize);
            prop.enum_blob_ptr = props.as_mut_ptr() as _;
            get(&mut prop)?;
            unsafe {
                props.set_len(prop.count_enum_blobs as usize);
            }
            let mut values = Vec::with_capacity(props.len());
            for v in props {
                values.push(DrmPropertyEnumValue {
                    value: v.value,
                    name: v.name.split(|n| *n == 0).next().unwrap().to_vec().into(),
                })
            }
            DrmPropertyType::Enum {
                values,
                bitmask: ty == DRM_MODE_PROP_BITMASK,
            }
        }
        DRM_MODE_PROP_BLOB => DrmPropertyType::Blob,
        DRM_MODE_PROP_OBJECT => {
            if prop.count_values != 1 {
                return Err(DrmError::ObjectValues);
            }
            let mut ty = 0u64;
            prop.values_ptr = &mut ty as *mut _ as u64;
            get(&mut prop)?;
            DrmPropertyType::Object { _ty: ty as _ }
        }
        _ => return Err(DrmError::UnknownPropertyType(ty)),
    };

    Ok(DrmPropertyDefinition {
        id: property_id,
        name: prop.name.split(|n| *n == 0).next().unwrap().to_vec().into(),
        _immutable: prop.flags.contains(DRM_MODE_PROP_IMMUTABLE),
        _atomic: prop.flags.contains(DRM_MODE_PROP_ATOMIC),
        ty,
    })
}

#[repr(C)]
#[derive(Debug)]
struct drm_mode_obj_get_properties {
    props_ptr: u64,
    prop_values_ptr: u64,
    count_props: u32,
    obj_id: u32,
    obj_type: u32,
}

const DRM_IOCTL_MODE_OBJ_GETPROPERTIES: u64 = drm_iowr::<drm_mode_obj_get_properties>(0xb9);

pub fn mode_obj_getproperties(
    fd: c::c_int,
    obj_id: u32,
    obj_type: u32,
) -> Result<Vec<DrmPropertyValue>, DrmError> {
    let mut props = drm_mode_obj_get_properties {
        props_ptr: 0,
        prop_values_ptr: 0,
        count_props: 0,
        obj_id,
        obj_type,
    };

    let get = |prop: &mut drm_mode_obj_get_properties| {
        unsafe {
            if let Err(e) = ioctl(fd, DRM_IOCTL_MODE_OBJ_GETPROPERTIES, prop) {
                return Err(DrmError::GetProperties(e));
            }
        }
        Ok(())
    };

    get(&mut props)?;

    let mut ids = Vec::<u32>::new();
    let mut values = Vec::<u64>::new();
    let mut num_props = 0;

    while num_props != props.count_props {
        num_props = props.count_props;

        ids.reserve(num_props as _);
        values.reserve(num_props as _);

        props.props_ptr = ids.as_mut_ptr() as _;
        props.prop_values_ptr = values.as_mut_ptr() as _;

        get(&mut props)?;
    }

    unsafe {
        ids.set_len(num_props as _);
        values.set_len(num_props as _);
    }

    let mut props = Vec::with_capacity(num_props as _);
    for (id, value) in ids.into_iter().zip(values.into_iter()) {
        props.push(DrmPropertyValue {
            id: DrmProperty(id),
            value,
        })
    }
    Ok(props)
}

pub const DRM_MODE_OBJECT_CRTC: u32 = 0xcccccccc;
pub const DRM_MODE_OBJECT_CONNECTOR: u32 = 0xc0c0c0c0;
pub const DRM_MODE_OBJECT_ENCODER: u32 = 0xe0e0e0e0;
pub const DRM_MODE_OBJECT_MODE: u32 = 0xdededede;
pub const DRM_MODE_OBJECT_PROPERTY: u32 = 0xb0b0b0b0;
pub const DRM_MODE_OBJECT_FB: u32 = 0xfbfbfbfb;
pub const DRM_MODE_OBJECT_BLOB: u32 = 0xbbbbbbbb;
pub const DRM_MODE_OBJECT_PLANE: u32 = 0xeeeeeeee;
#[expect(dead_code)]
pub const DRM_MODE_OBJECT_ANY: u32 = 0;

pub const DRM_MODE_CONNECTOR_Unknown: u32 = 0;
pub const DRM_MODE_CONNECTOR_VGA: u32 = 1;
pub const DRM_MODE_CONNECTOR_DVII: u32 = 2;
pub const DRM_MODE_CONNECTOR_DVID: u32 = 3;
pub const DRM_MODE_CONNECTOR_DVIA: u32 = 4;
pub const DRM_MODE_CONNECTOR_Composite: u32 = 5;
pub const DRM_MODE_CONNECTOR_SVIDEO: u32 = 6;
pub const DRM_MODE_CONNECTOR_LVDS: u32 = 7;
pub const DRM_MODE_CONNECTOR_Component: u32 = 8;
pub const DRM_MODE_CONNECTOR_9PinDIN: u32 = 9;
pub const DRM_MODE_CONNECTOR_DisplayPort: u32 = 10;
pub const DRM_MODE_CONNECTOR_HDMIA: u32 = 11;
pub const DRM_MODE_CONNECTOR_HDMIB: u32 = 12;
pub const DRM_MODE_CONNECTOR_TV: u32 = 13;
pub const DRM_MODE_CONNECTOR_eDP: u32 = 14;
pub const DRM_MODE_CONNECTOR_VIRTUAL: u32 = 15;
pub const DRM_MODE_CONNECTOR_DSI: u32 = 16;
pub const DRM_MODE_CONNECTOR_DPI: u32 = 17;
pub const DRM_MODE_CONNECTOR_WRITEBACK: u32 = 18;
pub const DRM_MODE_CONNECTOR_SPI: u32 = 19;
pub const DRM_MODE_CONNECTOR_USB: u32 = 20;

#[repr(C)]
struct drm_set_client_cap {
    capability: u64,
    value: u64,
}

const DRM_IOCTL_SET_CLIENT_CAP: u64 = drm_iow::<drm_set_client_cap>(0x0d);

pub const DRM_CLIENT_CAP_ATOMIC: u64 = 3;

pub fn set_client_cap(fd: c::c_int, capability: u64, value: u64) -> Result<(), OsError> {
    let mut cap = drm_set_client_cap { capability, value };
    unsafe {
        ioctl(fd, DRM_IOCTL_SET_CLIENT_CAP, &mut cap)?;
    }
    Ok(())
}

#[repr(C)]
struct drm_get_cap {
    capability: u64,
    value: u64,
}

const DRM_IOCTL_GET_CAP: u64 = drm_iowr::<drm_get_cap>(0x0c);

pub fn get_cap(fd: c::c_int, capability: u64) -> Result<u64, OsError> {
    let mut cap = drm_get_cap {
        capability,
        value: 0,
    };
    unsafe {
        ioctl(fd, DRM_IOCTL_GET_CAP, &mut cap)?;
    }
    Ok(cap.value)
}

#[repr(C)]
#[derive(Default)]
struct drm_mode_card_res {
    fb_id_ptr: u64,
    crtc_id_ptr: u64,
    connector_id_ptr: u64,
    encoder_id_ptr: u64,
    count_fbs: u32,
    count_crtcs: u32,
    count_connectors: u32,
    count_encoders: u32,
    min_width: u32,
    max_width: u32,
    min_height: u32,
    max_height: u32,
}

const DRM_IOCTL_MODE_GETRESOURCES: u64 = drm_iowr::<drm_mode_card_res>(0xa0);

pub fn mode_get_resources(fd: c::c_int) -> Result<DrmCardResources, DrmError> {
    let mut res = drm_mode_card_res::default();

    let get = |res: &mut drm_mode_card_res| {
        unsafe {
            if let Err(e) = ioctl(fd, DRM_IOCTL_MODE_GETRESOURCES, res) {
                return Err(DrmError::GetResources(e));
            }
        }
        Ok(())
    };

    get(&mut res)?;

    let mut count_fbs = 0;
    let mut count_crtcs = 0;
    let mut count_connectors = 0;
    let mut count_encoders = 0;

    let mut fbs = Vec::<DrmFb>::new();
    let mut crtcs = Vec::<DrmCrtc>::new();
    let mut connectors = Vec::<DrmConnector>::new();
    let mut encoders = Vec::<DrmEncoder>::new();

    while (count_fbs, count_crtcs, count_connectors, count_encoders)
        != (
            res.count_fbs,
            res.count_crtcs,
            res.count_connectors,
            res.count_encoders,
        )
    {
        count_fbs = res.count_fbs;
        count_crtcs = res.count_crtcs;
        count_connectors = res.count_connectors;
        count_encoders = res.count_encoders;

        fbs.reserve(count_fbs as _);
        crtcs.reserve(count_crtcs as _);
        connectors.reserve(count_connectors as _);
        encoders.reserve(count_encoders as _);

        res.fb_id_ptr = fbs.as_mut_ptr() as _;
        res.crtc_id_ptr = crtcs.as_mut_ptr() as _;
        res.connector_id_ptr = connectors.as_mut_ptr() as _;
        res.encoder_id_ptr = encoders.as_mut_ptr() as _;

        get(&mut res)?;
    }

    unsafe {
        fbs.set_len(count_fbs as _);
        crtcs.set_len(count_crtcs as _);
        connectors.set_len(count_connectors as _);
        encoders.set_len(count_encoders as _);
    }

    Ok(DrmCardResources {
        min_width: res.min_width,
        max_width: res.max_width,
        min_height: res.min_height,
        max_height: res.max_height,
        _fbs: fbs,
        crtcs,
        connectors,
        encoders,
    })
}

#[repr(C)]
struct drm_mode_get_plane_res {
    plane_id_ptr: u64,
    count_planes: u32,
}

const DRM_IOCTL_MODE_GETPLANERESOURCES: u64 = drm_iowr::<drm_mode_get_plane_res>(0xb5);

pub fn mode_getplaneresources(fd: c::c_int) -> Result<Vec<DrmPlane>, DrmError> {
    let mut res = drm_mode_get_plane_res {
        plane_id_ptr: 0,
        count_planes: 0,
    };

    let get = |res: &mut drm_mode_get_plane_res| {
        unsafe {
            if let Err(e) = ioctl(fd, DRM_IOCTL_MODE_GETPLANERESOURCES, res) {
                return Err(DrmError::GetPlaneResources(e));
            }
        }
        Ok(())
    };

    get(&mut res)?;

    let mut count_planes = 0;
    let mut planes = Vec::<DrmPlane>::new();

    while count_planes != res.count_planes {
        count_planes = res.count_planes;
        planes.reserve(count_planes as _);
        res.plane_id_ptr = planes.as_mut_ptr() as _;
        get(&mut res)?;
    }

    unsafe {
        planes.set_len(count_planes as _);
    }

    Ok(planes)
}

#[repr(C)]
#[derive(Default)]
struct drm_mode_get_plane {
    plane_id: u32,

    crtc_id: u32,
    fb_id: u32,

    possible_crtcs: u32,
    gamma_size: u32,

    count_format_types: u32,
    format_type_ptr: u64,
}

const DRM_IOCTL_MODE_GETPLANE: u64 = drm_iowr::<drm_mode_get_plane>(0xb6);

pub fn mode_getplane(fd: c::c_int, plane_id: u32) -> Result<DrmPlaneInfo, DrmError> {
    let mut res = drm_mode_get_plane {
        plane_id,
        ..Default::default()
    };

    let get = |res: &mut drm_mode_get_plane| {
        unsafe {
            if let Err(e) = ioctl(fd, DRM_IOCTL_MODE_GETPLANE, res) {
                return Err(DrmError::GetPlane(e));
            }
        }
        Ok(())
    };

    get(&mut res)?;

    let mut count_formats = 0;
    let mut formats = Vec::<u32>::new();

    while count_formats != res.count_format_types {
        count_formats = res.count_format_types;
        formats.reserve(count_formats as _);
        res.format_type_ptr = formats.as_mut_ptr() as _;
        get(&mut res)?;
    }

    unsafe {
        formats.set_len(count_formats as _);
    }

    Ok(DrmPlaneInfo {
        _plane_id: DrmPlane(plane_id),
        _crtc_id: DrmCrtc(res.crtc_id),
        _fb_id: DrmFb(res.fb_id),
        possible_crtcs: res.possible_crtcs,
        _gamma_size: res.gamma_size,
        format_types: formats,
    })
}

#[repr(C)]
#[derive(Default)]
struct drm_mode_get_encoder {
    encoder_id: u32,
    encoder_type: u32,

    crtc_id: u32,

    possible_crtcs: u32,
    possible_clones: u32,
}

const DRM_IOCTL_MODE_GETENCODER: u64 = drm_iowr::<drm_mode_get_encoder>(0xa6);

pub fn mode_getencoder(fd: c::c_int, encoder_id: u32) -> Result<DrmEncoderInfo, DrmError> {
    let mut res = drm_mode_get_encoder {
        encoder_id,
        ..Default::default()
    };

    unsafe {
        if let Err(e) = ioctl(fd, DRM_IOCTL_MODE_GETENCODER, &mut res) {
            return Err(DrmError::GetEncoder(e));
        }
    }

    Ok(DrmEncoderInfo {
        _encoder_id: DrmEncoder(encoder_id),
        _encoder_type: res.encoder_type,
        _crtc_id: DrmCrtc(res.crtc_id),
        possible_crtcs: res.possible_crtcs,
        _possible_clones: res.possible_clones,
    })
}

pub const DRM_DISPLAY_MODE_LEN: usize = 32;

#[repr(C)]
#[derive(Debug)]
pub struct drm_mode_modeinfo {
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
    pub name: [u8; DRM_DISPLAY_MODE_LEN],
}

unsafe impl Pod for drm_mode_modeinfo {}

impl Into<DrmModeInfo> for drm_mode_modeinfo {
    fn into(self) -> DrmModeInfo {
        DrmModeInfo {
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
            name: self.name.split(|n| *n == 0).next().unwrap().to_vec().into(),
        }
    }
}

pub const CONNECTOR_STATUS_CONNECTED: u32 = 1;
pub const CONNECTOR_STATUS_DISCONNECTED: u32 = 2;
pub const CONNECTOR_STATUS_UNKNOWN: u32 = 3;

#[derive(Default)]
#[repr(C)]
struct drm_mode_get_connector {
    encoders_ptr: u64,
    modes_ptr: u64,
    props_ptr: u64,
    prop_values_ptr: u64,

    count_modes: u32,
    count_props: u32,
    count_encoders: u32,

    encoder_id: u32,
    connector_id: u32,
    connector_type: u32,
    connector_type_id: u32,

    connection: u32,
    mm_width: u32,
    mm_height: u32,
    subpixel: u32,

    pad: u32,
}

const DRM_IOCTL_MODE_GETCONNECTOR: u64 = drm_iowr::<drm_mode_get_connector>(0xa7);

pub fn mode_getconnector(
    fd: c::c_int,
    connector: u32,
    force: bool,
) -> Result<DrmConnectorInfo, DrmError> {
    let mut count_modes = if force { 0 } else { 1 };
    let mut count_props = 0;
    let mut count_encoders = 0;

    let mut modes = Vec::<drm_mode_modeinfo>::with_capacity(1);
    let mut props = Vec::<u32>::new();
    let mut prop_values = Vec::<u64>::new();
    let mut encoders = Vec::<DrmEncoder>::new();

    let mut res = drm_mode_get_connector {
        connector_id: connector,
        count_modes,
        modes_ptr: modes.as_mut_ptr() as _,
        ..Default::default()
    };

    let get = |res: &mut drm_mode_get_connector| {
        unsafe {
            if let Err(e) = ioctl(fd, DRM_IOCTL_MODE_GETCONNECTOR, res) {
                return Err(DrmError::GetConnector(e));
            }
        }
        Ok(())
    };

    get(&mut res)?;

    while (count_modes, count_props, count_encoders)
        != (res.count_modes, res.count_props, res.count_encoders)
    {
        count_modes = res.count_modes;
        count_props = res.count_props;
        count_encoders = res.count_encoders;

        modes.reserve(count_modes as _);
        props.reserve(count_props as _);
        prop_values.reserve(count_props as _);
        encoders.reserve(count_encoders as _);

        res.modes_ptr = modes.as_mut_ptr() as _;
        res.props_ptr = props.as_mut_ptr() as _;
        res.prop_values_ptr = prop_values.as_mut_ptr() as _;
        res.encoders_ptr = encoders.as_mut_ptr() as _;

        get(&mut res)?;
    }

    unsafe {
        modes.set_len(count_modes as _);
        props.set_len(count_props as _);
        prop_values.set_len(count_props as _);
        encoders.set_len(count_encoders as _);
    }

    Ok(DrmConnectorInfo {
        encoders,
        modes: modes.into_iter().map(|m| m.into()).collect(),
        _props: props
            .into_iter()
            .zip(prop_values)
            .map(|(id, value)| DrmPropertyValue {
                id: DrmProperty(id),
                value,
            })
            .collect(),
        _encoder_id: DrmEncoder(res.encoder_id),
        _connector_id: DrmConnector(res.connector_id),
        connector_type: res.connector_type,
        connector_type_id: res.connector_type_id,
        connection: res.connection,
        mm_width: res.mm_width,
        mm_height: res.mm_height,
        subpixel: res.subpixel,
    })
}

#[repr(C)]
struct drm_mode_atomic {
    flags: u32,
    count_objs: u32,
    objs_ptr: u64,
    count_props_ptr: u64,
    props_ptr: u64,
    prop_values_ptr: u64,
    reserved: u64,
    user_data: u64,
}

const DRM_IOCTL_MODE_ATOMIC: u64 = drm_iowr::<drm_mode_atomic>(0xbc);

pub const DRM_MODE_PAGE_FLIP_EVENT: u32 = 0x01;
pub const DRM_MODE_PAGE_FLIP_ASYNC: u32 = 0x02;
pub const DRM_MODE_ATOMIC_TEST_ONLY: u32 = 0x0100;
pub const DRM_MODE_ATOMIC_NONBLOCK: u32 = 0x0200;
pub const DRM_MODE_ATOMIC_ALLOW_MODESET: u32 = 0x0400;

pub fn mode_atomic(
    fd: c::c_int,
    flags: u32,
    objs: &[u32],
    count_props: &[u32],
    props: &[u32],
    prop_values: &[u64],
    user_data: u64,
) -> Result<(), DrmError> {
    assert_eq!(objs.len(), count_props.len());
    assert_eq!(props.len(), prop_values.len());
    assert_eq!(
        count_props.iter().copied().sum::<u32>() as usize,
        props.len()
    );

    if objs.is_empty() {
        return Ok(());
    }

    let mut req = drm_mode_atomic {
        flags,
        count_objs: objs.len() as _,
        objs_ptr: objs.as_ptr() as _,
        count_props_ptr: count_props.as_ptr() as _,
        props_ptr: props.as_ptr() as _,
        prop_values_ptr: prop_values.as_ptr() as _,
        reserved: 0,
        user_data,
    };

    unsafe {
        if let Err(e) = ioctl(fd, DRM_IOCTL_MODE_ATOMIC, &mut req) {
            return Err(DrmError::Atomic(e));
        }
    }
    Ok(())
}

#[repr(C)]
struct drm_mode_create_blob {
    data: u64,
    length: u32,
    blob_id: u32,
}

const DRM_IOCTL_MODE_CREATEPROPBLOB: u64 = drm_iowr::<drm_mode_create_blob>(0xbd);

pub fn mode_create_blob<T>(fd: c::c_int, t: &T) -> Result<DrmBlob, OsError> {
    let mut res = drm_mode_create_blob {
        data: t as *const T as _,
        length: size_of_val(t) as _,
        blob_id: 0,
    };

    unsafe {
        ioctl(fd, DRM_IOCTL_MODE_CREATEPROPBLOB, &mut res)?;
    }
    Ok(DrmBlob(res.blob_id))
}

#[repr(C)]
struct drm_mode_destroy_blob {
    blob_id: u32,
}

const DRM_IOCTL_MODE_DESTROYPROPBLOB: u64 = drm_iowr::<drm_mode_destroy_blob>(0xbe);

pub fn mode_destroy_blob(fd: c::c_int, id: DrmBlob) -> Result<(), OsError> {
    let mut res = drm_mode_destroy_blob { blob_id: id.0 };

    unsafe {
        ioctl(fd, DRM_IOCTL_MODE_DESTROYPROPBLOB, &mut res)?;
    }
    Ok(())
}

#[repr(C)]
#[derive(Debug)]
struct drm_mode_fb_cmd2 {
    fb_id: u32,
    width: u32,
    height: u32,
    pixel_format: u32,
    flags: u32,
    handles: [u32; 4],
    pitches: [u32; 4],
    offsets: [u32; 4],
    modifiers: [u64; 4],
}

#[expect(dead_code)]
pub const DRM_MODE_FB_INTERLACED: u32 = 1 << 0;
pub const DRM_MODE_FB_MODIFIERS: u32 = 1 << 1;

const DRM_IOCTL_MODE_ADDFB2: u64 = drm_iowr::<drm_mode_fb_cmd2>(0xb8);

pub fn mode_addfb2(
    fd: c::c_int,
    width: u32,
    height: u32,
    pixel_format: u32,
    flags: u32,
    handles: [u32; 4],
    strides: [u32; 4],
    offsets: [u32; 4],
    modifiers: [u64; 4],
) -> Result<DrmFb, OsError> {
    let mut res = drm_mode_fb_cmd2 {
        fb_id: 0,
        width,
        height,
        pixel_format,
        flags,
        handles,
        pitches: strides,
        offsets,
        modifiers,
    };
    // log::info!("{:#?}", res);

    unsafe {
        ioctl(fd, DRM_IOCTL_MODE_ADDFB2, &mut res)?;
    }

    Ok(DrmFb(res.fb_id))
}

const DRM_IOCTL_MODE_RMFB: u64 = drm_iowr::<c::c_uint>(0xaf);

pub fn mode_rmfb(fd: c::c_int, id: DrmFb) -> Result<(), OsError> {
    let mut res = id.0 as c::c_uint;
    unsafe {
        ioctl(fd, DRM_IOCTL_MODE_RMFB, &mut res)?;
    }
    Ok(())
}

#[repr(C)]
struct drm_prime_handle {
    handle: u32,
    flags: u32,
    fd: i32,
}

const DRM_IOCTL_PRIME_FD_TO_HANDLE: u64 = drm_iowr::<drm_prime_handle>(0x2e);

pub fn prime_fd_to_handle(fd: c::c_int, prime: c::c_int) -> Result<u32, OsError> {
    let mut res = drm_prime_handle {
        handle: 0,
        flags: 0,
        fd: prime,
    };
    unsafe {
        ioctl(fd, DRM_IOCTL_PRIME_FD_TO_HANDLE, &mut res)?;
    }
    Ok(res.handle)
}

#[repr(C)]
struct drm_gem_close {
    handle: u32,
    pad: u32,
}

const DRM_IOCTL_GEM_CLOSE: u64 = drm_iow::<drm_gem_close>(0x09);

pub fn gem_close(fd: c::c_int, handle: u32) -> Result<(), OsError> {
    let mut res = drm_gem_close { handle, pad: 0 };
    unsafe {
        ioctl(fd, DRM_IOCTL_GEM_CLOSE, &mut res)?;
    }
    Ok(())
}

#[expect(dead_code)]
pub const DRM_EVENT_VBLANK: u32 = 0x01;
pub const DRM_EVENT_FLIP_COMPLETE: u32 = 0x02;
pub const DRM_EVENT_CRTC_SEQUENCE: u32 = 0x03;

#[repr(C)]
pub struct drm_event {
    pub ty: u32,
    pub length: u32,
}

unsafe impl Pod for drm_event {}

#[repr(C)]
pub struct drm_event_vblank {
    pub base: drm_event,
    pub user_data: u64,
    pub tv_sec: u32,
    pub tv_usec: u32,
    pub sequence: u32,
    pub crtc_id: u32,
}

unsafe impl Pod for drm_event_vblank {}

#[repr(C)]
pub struct drm_event_crtc_sequence {
    pub base: drm_event,
    pub user_data: u64,
    pub time_ns: i64,
    pub sequence: u64,
}

unsafe impl Pod for drm_event_crtc_sequence {}

#[repr(C)]
struct drm_mode_get_blob {
    blob_id: u32,
    length: u32,
    data: u64,
}

const DRM_IOCTL_MODE_GETPROPBLOB: u64 = drm_iowr::<drm_mode_get_blob>(0xac);

pub fn mode_getprobblob<T: Pod + ?Sized>(
    fd: c::c_int,
    blob_id: u32,
    t: &mut T,
) -> Result<usize, OsError> {
    let mut res = drm_mode_get_blob {
        blob_id,
        length: size_of_val(t) as _,
        data: t as *const T as *const u8 as _,
    };
    unsafe {
        ioctl(fd, DRM_IOCTL_MODE_GETPROPBLOB, &mut res)?;
    }
    Ok(res.length as _)
}

#[repr(C)]
struct drm_version {
    version_major: c::c_int,
    version_minor: c::c_int,
    version_patchlevel: c::c_int,
    name_len: usize, // actually __kernel_size_t but nobody cares about x32
    name: *mut u8,
    date_len: usize,
    date: *mut u8,
    desc_len: usize,
    desc: *mut u8,
}

unsafe impl Pod for drm_version {}

const DRM_IOCTL_VERSION: u64 = drm_iowr::<drm_version>(0x00);

pub fn get_version(fd: c::c_int) -> Result<DrmVersion, OsError> {
    let mut name = Vec::<u8>::new();
    let mut date = Vec::<u8>::new();
    let mut desc = Vec::<u8>::new();
    let mut res: drm_version = pod_zeroed();
    loop {
        res.name_len = name.capacity();
        res.name = name.as_mut_ptr();
        res.date_len = date.capacity();
        res.date = date.as_mut_ptr();
        res.desc_len = desc.capacity();
        res.desc = desc.as_mut_ptr();
        unsafe {
            ioctl(fd, DRM_IOCTL_VERSION, &mut res)?;
        }
        if res.name_len <= name.capacity()
            && res.date_len <= date.capacity()
            && res.desc_len <= desc.capacity()
        {
            break;
        }
        name.reserve_exact(res.name_len);
        date.reserve_exact(res.date_len);
        desc.reserve_exact(res.desc_len);
    }
    unsafe {
        name.set_len(res.name_len);
        date.set_len(res.date_len);
        desc.set_len(res.desc_len);
    }
    Ok(DrmVersion {
        version_major: res.version_major,
        version_minor: res.version_minor,
        version_patchlevel: res.version_patchlevel,
        name: name.into(),
        date: date.into(),
        desc: desc.into(),
    })
}

pub const FORMAT_BLOB_CURRENT: u32 = 1;

#[repr(C)]
pub struct drm_format_modifier_blob {
    pub version: u32,
    pub flags: u32,
    pub count_formats: u32,
    pub formats_offset: u32,
    pub count_modifiers: u32,
    pub modifiers_offset: u32,
}

unsafe impl Pod for drm_format_modifier_blob {}

#[repr(C)]
pub struct drm_format_modifier {
    pub formats: u64,
    pub offset: u32,
    pub pad: u32,
    pub modifier: u64,
}

unsafe impl Pod for drm_format_modifier {}

pub const DRM_SYNCOBJ_CREATE_SIGNALED: u32 = 1 << 0;

#[repr(C)]
struct drm_syncobj_create {
    handle: u32,
    flags: u32,
}

const DRM_IOCTL_SYNCOBJ_CREATE: u64 = drm_iowr::<drm_syncobj_create>(0xBF);

pub fn sync_obj_create(drm: c::c_int, flags: u32) -> Result<u32, OsError> {
    let mut res = drm_syncobj_create { handle: 0, flags };
    unsafe {
        ioctl(drm, DRM_IOCTL_SYNCOBJ_CREATE, &mut res)?;
    }
    Ok(res.handle)
}

#[repr(C)]
struct drm_syncobj_destroy {
    handle: u32,
    pad: u32,
}

const DRM_IOCTL_SYNCOBJ_DESTROY: u64 = drm_iowr::<drm_syncobj_destroy>(0xC0);

pub fn sync_obj_destroy(drm: c::c_int, handle: u32) -> Result<(), OsError> {
    let mut res = drm_syncobj_destroy { handle, pad: 0 };
    unsafe {
        ioctl(drm, DRM_IOCTL_SYNCOBJ_DESTROY, &mut res)?;
    }
    Ok(())
}

pub const DRM_SYNCOBJ_FD_TO_HANDLE_FLAGS_IMPORT_SYNC_FILE: u32 = 1 << 0;
pub const DRM_SYNCOBJ_HANDLE_TO_FD_FLAGS_EXPORT_SYNC_FILE: u32 = 1 << 0;

#[repr(C)]
struct drm_syncobj_handle {
    handle: u32,
    flags: u32,
    fd: i32,
    pad: u32,
}

const DRM_IOCTL_SYNCOBJ_HANDLE_TO_FD: u64 = drm_iowr::<drm_syncobj_handle>(0xC1);
const DRM_IOCTL_SYNCOBJ_FD_TO_HANDLE: u64 = drm_iowr::<drm_syncobj_handle>(0xC2);

pub fn sync_obj_handle_to_fd(drm: c::c_int, handle: u32, flags: u32) -> Result<OwnedFd, OsError> {
    let mut res = drm_syncobj_handle {
        handle,
        flags,
        fd: 0,
        pad: 0,
    };
    unsafe {
        ioctl(drm, DRM_IOCTL_SYNCOBJ_HANDLE_TO_FD, &mut res)?;
    }
    Ok(OwnedFd::new(res.fd))
}

pub fn sync_obj_fd_to_handle(
    drm: c::c_int,
    fd: c::c_int,
    flags: u32,
    handle: u32,
) -> Result<u32, OsError> {
    let mut res = drm_syncobj_handle {
        handle,
        flags,
        fd,
        pad: 0,
    };
    unsafe {
        ioctl(drm, DRM_IOCTL_SYNCOBJ_FD_TO_HANDLE, &mut res)?;
    }
    Ok(res.handle)
}

// pub const DRM_SYNCOBJ_WAIT_FLAGS_WAIT_ALL: u32 = 1 << 0;
// pub const DRM_SYNCOBJ_WAIT_FLAGS_WAIT_FOR_SUBMIT: u32 = 1 << 1;
pub const DRM_SYNCOBJ_WAIT_FLAGS_WAIT_AVAILABLE: u32 = 1 << 2;

#[repr(C)]
struct drm_syncobj_eventfd {
    handle: u32,
    flags: u32,
    point: u64,
    fd: i32,
    pad: u32,
}

const DRM_IOCTL_SYNCOBJ_EVENTFD: u64 = drm_iowr::<drm_syncobj_eventfd>(0xCF);

pub fn sync_obj_eventfd(
    drm: c::c_int,
    eventfd: c::c_int,
    handle: u32,
    point: u64,
    flags: u32,
) -> Result<(), OsError> {
    let mut res = drm_syncobj_eventfd {
        handle,
        flags,
        point,
        fd: eventfd,
        pad: 0,
    };
    unsafe {
        ioctl(drm, DRM_IOCTL_SYNCOBJ_EVENTFD, &mut res)?;
    }
    Ok(())
}

#[repr(C)]
struct drm_syncobj_timeline_array {
    handles: u64,
    points: u64,
    count_handles: u32,
    flags: u32,
}

const DRM_IOCTL_SYNCOBJ_TIMELINE_SIGNAL: u64 = drm_iowr::<drm_syncobj_timeline_array>(0xCD);

pub fn sync_obj_signal(drm: c::c_int, handle: u32, point: u64) -> Result<(), OsError> {
    let mut res = drm_syncobj_timeline_array {
        handles: &handle as *const u32 as u64,
        points: &point as *const u64 as u64,
        count_handles: 1,
        flags: 0,
    };
    unsafe {
        ioctl(drm, DRM_IOCTL_SYNCOBJ_TIMELINE_SIGNAL, &mut res)?;
    }
    Ok(())
}

#[repr(C)]
struct drm_syncobj_transfer {
    src_handle: u32,
    dst_handle: u32,
    src_point: u64,
    dst_point: u64,
    flags: u32,
    pad: u32,
}

const DRM_IOCTL_SYNCOBJ_TRANSFER: u64 = drm_iowr::<drm_syncobj_transfer>(0xCC);

pub fn sync_obj_transfer(
    drm: c::c_int,
    src_handle: u32,
    src_point: u64,
    dst_handle: u32,
    dst_point: u64,
    flags: u32,
) -> Result<(), OsError> {
    let mut res = drm_syncobj_transfer {
        src_handle,
        dst_handle,
        src_point,
        dst_point,
        flags,
        pad: 0,
    };
    unsafe {
        ioctl(drm, DRM_IOCTL_SYNCOBJ_TRANSFER, &mut res)?;
    }
    Ok(())
}

#[repr(C)]
struct sync_merge_data {
    name: [u8; 32],
    fd2: i32,
    fence: i32,
    flags: u32,
    pad: u32,
}

const SYNC_IOC_MAGIC: u64 = b'>' as _;

const SYNC_IOC_MERGE: u64 = uapi::_IOWR::<sync_merge_data>(SYNC_IOC_MAGIC, 3);

pub fn sync_ioc_merge(left: c::c_int, right: c::c_int) -> Result<OwnedFd, OsError> {
    let mut res = sync_merge_data {
        name: [0; 32],
        fd2: right,
        fence: 0,
        flags: 0,
        pad: 0,
    };
    unsafe {
        ioctl(left, SYNC_IOC_MERGE, &mut res)?;
    }
    Ok(OwnedFd::new(res.fence))
}

const DRM_IOCTL_DROP_MASTER: u64 = drm_io(0x1f);

pub fn drop_master(fd: c::c_int) -> Result<(), OsError> {
    let mut res = 0u8;
    unsafe {
        ioctl(fd, DRM_IOCTL_DROP_MASTER, &mut res)?;
    }
    Ok(())
}

const DRM_IOCTL_AUTH_MAGIC: u64 = drm_iow::<drm_auth>(0x11);

#[repr(C)]
struct drm_auth {
    magic: c::c_uint,
}

pub fn auth_magic(fd: c::c_int, magic: c::c_uint) -> Result<(), OsError> {
    let mut res = drm_auth { magic };
    unsafe {
        ioctl(fd, DRM_IOCTL_AUTH_MAGIC, &mut res)?;
    }
    Ok(())
}

const DRM_CRTC_SEQUENCE_RELATIVE: u32 = 0x00000001;
// const DRM_CRTC_SEQUENCE_NEXT_ON_MISS: u32 =		0x00000002;

#[repr(C)]
struct drm_crtc_queue_sequence {
    crtc_id: u32,
    flags: u32,
    sequence: u64,
    user_data: u64,
}

const DRM_IOCTL_CRTC_QUEUE_SEQUENCE: u64 = drm_iowr::<drm_crtc_queue_sequence>(0x3c);

pub fn queue_sequence(fd: c::c_int, crtc: DrmCrtc) -> Result<(), OsError> {
    let mut res = drm_crtc_queue_sequence {
        crtc_id: crtc.0,
        flags: DRM_CRTC_SEQUENCE_RELATIVE,
        sequence: 1,
        user_data: crtc.0 as _,
    };
    unsafe {
        ioctl(fd, DRM_IOCTL_CRTC_QUEUE_SEQUENCE, &mut res)?;
    }
    Ok(())
}
