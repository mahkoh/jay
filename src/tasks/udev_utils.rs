use {
    crate::{
        udev::{Udev, UdevDeviceType},
        utils::errorfmt::ErrorFmt,
    },
    jay_config::PciId,
    std::rc::Rc,
    uapi::c,
};

#[derive(Default, Debug)]
pub struct UdevProps {
    pub syspath: Option<String>,
    pub devnode: Option<String>,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub pci_id: Option<PciId>,
}

pub fn udev_props(dev_t: c::dev_t, depth: usize) -> UdevProps {
    let mut res = UdevProps::default();
    let udev = match Udev::new() {
        Ok(udev) => Rc::new(udev),
        Err(e) => {
            log::error!("Could not create a udev instance: {}", e);
            return res;
        }
    };
    let mut dev = match udev.create_device_from_devnum(UdevDeviceType::Character, dev_t) {
        Ok(dev) => dev,
        Err(e) => {
            log::error!("{}", ErrorFmt(e));
            return res;
        }
    };
    res.devnode = dev.devnode().map(|s| s.to_string_lossy().into_owned());
    for _ in 0..depth {
        dev = match dev.parent() {
            Ok(dev) => dev,
            Err(e) => {
                log::error!("{}", ErrorFmt(e));
                return res;
            }
        }
    }
    res.syspath = dev.syspath().map(|s| s.to_string_lossy().into_owned());
    res.vendor = dev.vendor().map(|s| s.to_string_lossy().into_owned());
    res.model = dev.model().map(|s| s.to_string_lossy().into_owned());
    {
        let id = match dev.pci_id() {
            Some(id) => id,
            _ => return res,
        };
        let id = id.to_string_lossy();
        let colon = match id.find(':') {
            Some(pos) => pos,
            _ => return res,
        };
        let vendor = &id[..colon];
        let model = &id[colon + 1..];
        let vendor = match u32::from_str_radix(vendor, 16) {
            Ok(v) => v,
            _ => return res,
        };
        let model = match u32::from_str_radix(model, 16) {
            Ok(v) => v,
            _ => return res,
        };
        res.pci_id = Some(PciId { vendor, model });
    }
    res
}
