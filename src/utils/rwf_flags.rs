use std::sync::LazyLock;
use uapi::c;

#[expect(unused)]
pub const RWF_HIPRI: c::c_int = 0x00000001;
#[expect(unused)]
pub const RWF_DSYNC: c::c_int = 0x00000002;
#[expect(unused)]
pub const RWF_SYNC: c::c_int = 0x00000004;
#[expect(unused)]
pub const RWF_NOWAIT: c::c_int = 0x00000008;
#[expect(unused)]
pub const RWF_APPEND: c::c_int = 0x00000010;
#[expect(unused)]
pub const RWF_NOAPPEND: c::c_int = 0x00000020;
#[expect(unused)]
pub const RWF_ATOMIC: c::c_int = 0x00000040;
#[expect(unused)]
pub const RWF_DONTCACHE: c::c_int = 0x00000080;
pub const RWF_NOSIGNAL: c::c_int = 0x00000100;

pub fn supports_rwf_nosignal() -> bool {
    static V: LazyLock<bool> = LazyLock::new(|| {
        let ok = supports_rwf_nosignal_();
        if !ok {
            log::warn!("Kernel does not support RWF_NOSIGNAL");
        }
        ok
    });
    *V
}

fn supports_rwf_nosignal_() -> bool {
    let Ok(f) = uapi::memfd_create("", c::MFD_CLOEXEC) else {
        return false;
    };
    let bufs = &[&[0u8][..]][..];
    uapi::pwritev2(f.raw(), bufs, 0, RWF_NOSIGNAL).is_ok()
}
