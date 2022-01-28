use crate::utils::bitflags::BitflagsExt;
use crate::utils::debug_fn::debug_fn;
use crate::utils::ptr_ext::PtrExt;
use bstr::ByteSlice;
use std::ffi::{CStr, CString};
use std::fmt::{Debug, Formatter};
use std::ptr;
use thiserror::Error;
use uapi::c::c_char;
use uapi::{c, Errno, OwnedFd, Ustring};

#[derive(Debug, Error)]
pub enum DrmError {
    #[error("Could not create a lease")]
    CreateLeaseError(#[source] std::io::Error),
    #[error("Could not reopen a node")]
    ReopenNode(#[source] std::io::Error),
    #[error("Could not retrieve the render node name")]
    RenderNodeName,
    #[error("Could not retrieve the device node name")]
    DeviceNodeName,
    #[error("Could not retrieve device")]
    GetDevice(#[source] std::io::Error),
}

const DRM_NODE_PRIMARY: c::c_int = 0;
const DRM_NODE_CONTROL: c::c_int = 1;
const DRM_NODE_RENDER: c::c_int = 2;
const DRM_NODE_MAX: c::c_int = 3;

const DRM_BUS_PCI: c::c_int = 0;
const DRM_BUS_USB: c::c_int = 1;
const DRM_BUS_PLATFORM: c::c_int = 2;
const DRM_BUS_HOST1X: c::c_int = 3;

#[link(name = "drm")]
extern "C" {
    fn drmIsMaster(fd: c::c_int) -> c::c_int;
    fn drmModeCreateLease(
        fd: c::c_int,
        o: *const u32,
        num_objects: c::c_int,
        flags: c::c_int,
        lessee_id: *mut u32,
    ) -> c::c_int;
    fn drmGetNodeTypeFromFd(fd: c::c_int) -> c::c_int;
    fn drmGetRenderDeviceNameFromFd(fd: c::c_int) -> *mut c::c_char;
    fn drmGetDeviceNameFromFd2(fd: c::c_int) -> *mut c::c_char;
    fn drmFreeDevice(device: *mut *mut drmDevice);
    fn drmGetDevice(fd: c::c_int, device: *mut *mut drmDevice) -> c::c_int;
}

fn render_node_name(fd: c::c_int) -> Result<Ustring, DrmError> {
    unsafe {
        let name = drmGetRenderDeviceNameFromFd(fd);
        if name.is_null() {
            Err(DrmError::RenderNodeName)
        } else {
            Ok(CString::from_raw(name).into())
        }
    }
}

fn device_node_name(fd: c::c_int) -> Result<Ustring, DrmError> {
    unsafe {
        let name = drmGetDeviceNameFromFd2(fd);
        if name.is_null() {
            Err(DrmError::DeviceNodeName)
        } else {
            Ok(CString::from_raw(name).into())
        }
    }
}

fn reopen(fd: c::c_int, allow_downgrade: bool) -> Result<OwnedFd, DrmError> {
    unsafe {
        if drmIsMaster(fd) != 0 {
            let mut lessee = 0;
            let lease_fd = drmModeCreateLease(fd, ptr::null(), 0, c::O_CLOEXEC, &mut lessee);
            if lease_fd >= 0 {
                return Ok(OwnedFd::new(lease_fd));
            }
        }
        let path = if drmGetNodeTypeFromFd(fd) == DRM_NODE_RENDER {
            uapi::format_ustr!("/proc/self/fd/{}", fd)
        } else if allow_downgrade {
            render_node_name(fd)?
        } else {
            device_node_name(fd)?
        };
        match uapi::open(&path, c::O_RDWR | c::O_CLOEXEC, 0) {
            Ok(f) => Ok(f),
            Err(e) => Err(DrmError::ReopenNode(e.into())),
        }
    }
}

pub struct Drm {
    fd: OwnedFd,
}

impl Drm {
    pub fn new(fd: c::c_int, allow_downgrade: bool) -> Result<Self, DrmError> {
        Ok(Self {
            fd: reopen(fd, allow_downgrade)?,
        })
    }

    pub fn raw(&self) -> c::c_int {
        self.fd.raw()
    }

    pub fn dup_unprivileged(&self) -> Result<Self, DrmError> {
        Self::new(self.fd.raw(), true)
    }

    pub fn get_device(&self) -> Result<DrmDevice, DrmError> {
        unsafe {
            let mut dev = ptr::null_mut();
            if drmGetDevice(self.fd.raw(), &mut dev) < 0 {
                return Err(DrmError::GetDevice(Errno::default().into()));
            }
            Ok(DrmDevice { dev })
        }
    }
}

#[repr(C)]
struct drmPciBusInfo {
    domain: u16,
    bus: u8,
    dev: u8,
    func: u8,
}

#[repr(C)]
struct drmUsbBusInfo {
    bus: u8,
    dev: u8,
}

const DRM_PLATFORM_DEVICE_NAME_LEN: usize = 512;

#[repr(C)]
struct drmPlatformBusInfo {
    fullname: [c::c_char; DRM_PLATFORM_DEVICE_NAME_LEN],
}

const DRM_HOST1X_DEVICE_NAME_LEN: usize = 512;

#[repr(C)]
struct drmHost1xBusInfo {
    fullname: [c::c_char; DRM_HOST1X_DEVICE_NAME_LEN],
}

#[repr(C)]
union businfo {
    pci: *mut drmPciBusInfo,
    usb: *mut drmUsbBusInfo,
    platform: *mut drmPlatformBusInfo,
    host1x: *mut drmHost1xBusInfo,
}

#[repr(C)]
struct drmPciDeviceInfo {
    vendor_id: u16,
    device_id: u16,
    subvendor_id: u16,
    subdevice_id: u16,
    revision_id: u8,
}

#[repr(C)]
struct drmUsbDeviceInfo {
    vendor: u16,
    product: u16,
}

#[repr(C)]
struct drmPlatformDeviceInfo {
    compatible: *mut *mut c::c_char,
}

#[repr(C)]
struct drmHost1xDeviceInfo {
    compatible: *mut *mut c::c_char,
}

#[repr(C)]
union deviceinfo {
    pci: *mut drmPciDeviceInfo,
    usb: *mut drmUsbDeviceInfo,
    platform: *mut drmPlatformDeviceInfo,
    host1x: *mut drmHost1xDeviceInfo,
}

#[repr(C)]
struct drmDevice {
    nodes: *mut *mut c::c_char,
    available_nodes: c::c_int,
    bustype: c::c_int,
    businfo: businfo,
    deviceinfo: deviceinfo,
}

pub struct DrmDevice {
    dev: *mut drmDevice,
}

impl Drop for DrmDevice {
    fn drop(&mut self) {
        unsafe {
            drmFreeDevice(&mut self.dev);
        }
    }
}

impl DrmDevice {
    pub fn nodes<'a>(&'a self) -> impl Iterator<Item = (c::c_int, &'a CStr)> + 'a {
        struct Iter<'a> {
            next: usize,
            dev: &'a DrmDevice,
        }
        impl<'a> Iterator for Iter<'a> {
            type Item = (c::c_int, &'a CStr);

            fn next(&mut self) -> Option<Self::Item> {
                unsafe {
                    let dev = self.dev.dev.deref();
                    while self.next < DRM_NODE_MAX as _ {
                        let idx = self.next;
                        self.next += 1;
                        if dev.available_nodes.contains(1 << idx) {
                            return Some((idx as _, CStr::from_ptr(*dev.nodes.add(idx))));
                        }
                    }
                    None
                }
            }
        }
        Iter { next: 0, dev: self }
    }
}

impl Debug for DrmDevice {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        struct StrStr<'a> {
            v: &'a [*mut c::c_char],
        }
        impl Debug for StrStr<'_> {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                let mut list = f.debug_list();
                for &v in self.v {
                    if v.is_null() {
                        list.entry(&v);
                    } else {
                        unsafe {
                            list.entry(&CStr::from_ptr(v));
                        }
                    }
                }
                list.finish()
            }
        }
        impl<'a> StrStr<'a> {
            fn from_nt(nt: *const *mut c_char) -> Self {
                unsafe {
                    let mut num = 0;
                    let mut tmp = nt;
                    while !tmp.deref().is_null() {
                        num += 1;
                        tmp = tmp.add(1);
                    }
                    Self {
                        v: std::slice::from_raw_parts(nt, num),
                    }
                }
            }
        }
        let mut ds = f.debug_struct("DrmDevice");
        unsafe {
            let dev = self.dev.deref();
            let nodes = std::slice::from_raw_parts(dev.nodes, DRM_NODE_MAX as _);
            ds.field(
                "available_nodes",
                &debug_fn(|f| write!(f, "0b{:b}", dev.available_nodes)),
            );
            ds.field("nodes", &StrStr { v: nodes });
            ds.field("bustype", &dev.bustype);
            match dev.bustype {
                DRM_BUS_PCI => {
                    ds.field(
                        "businfo",
                        &debug_fn(|f| {
                            let pci = dev.businfo.pci.deref();
                            f.debug_struct("drmPciBusInfo")
                                .field("domain", &pci.domain)
                                .field("bus", &pci.bus)
                                .field("dev", &pci.dev)
                                .field("func", &pci.func)
                                .finish()
                        }),
                    );
                    ds.field(
                        "deviceinfo",
                        &debug_fn(|f| {
                            let pci = dev.deviceinfo.pci.deref();
                            f.debug_struct("drmPciDeviceInfo")
                                .field("vendor_id", &pci.vendor_id)
                                .field("device_id", &pci.device_id)
                                .field("subvendor_id", &pci.subvendor_id)
                                .field("subdevice_id", &pci.subdevice_id)
                                .field("revision_id", &pci.revision_id)
                                .finish()
                        }),
                    );
                }
                DRM_BUS_USB => {
                    ds.field(
                        "businfo",
                        &debug_fn(|f| {
                            let usb = dev.businfo.usb.deref();
                            f.debug_struct("drmUsbBusInfo")
                                .field("bus", &usb.bus)
                                .field("dev", &usb.dev)
                                .finish()
                        }),
                    );
                    ds.field(
                        "deviceinfo",
                        &debug_fn(|f| {
                            let usb = dev.deviceinfo.usb.deref();
                            f.debug_struct("drmUsbDeviceInfo")
                                .field("vendor", &usb.vendor)
                                .field("product", &usb.product)
                                .finish()
                        }),
                    );
                }
                DRM_BUS_PLATFORM => {
                    ds.field(
                        "businfo",
                        &debug_fn(|f| {
                            let platform = dev.businfo.platform.deref();
                            f.debug_struct("drmPlatformBusInfo")
                                .field(
                                    "fullname",
                                    &CStr::from_ptr(platform.fullname.as_ptr())
                                        .to_bytes()
                                        .as_bstr(),
                                )
                                .finish()
                        }),
                    );
                    ds.field(
                        "deviceinfo",
                        &debug_fn(|f| {
                            let platform = dev.deviceinfo.platform.deref();
                            f.debug_struct("drmPlatformDeviceInfo")
                                .field("compatible", &StrStr::from_nt(platform.compatible))
                                .finish()
                        }),
                    );
                }
                DRM_BUS_HOST1X => {
                    ds.field(
                        "businfo",
                        &debug_fn(|f| {
                            let host1x = dev.businfo.host1x.deref();
                            f.debug_struct("drmHost1xBusInfo")
                                .field(
                                    "fullname",
                                    &CStr::from_ptr(host1x.fullname.as_ptr())
                                        .to_bytes()
                                        .as_bstr(),
                                )
                                .finish()
                        }),
                    );
                    ds.field(
                        "deviceinfo",
                        &debug_fn(|f| {
                            let host1x = dev.deviceinfo.host1x.deref();
                            f.debug_struct("drmHost1xDeviceInfo")
                                .field("compatible", &StrStr::from_nt(host1x.compatible))
                                .finish()
                        }),
                    );
                }
                _ => {}
            }
            ds.finish()
        }
    }
}
