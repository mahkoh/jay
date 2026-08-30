use std::sync::LazyLock;
use uapi::c;

pub fn uid() -> c::uid_t {
    static V: LazyLock<c::uid_t> = LazyLock::new(uapi::getuid);
    *V
}

pub fn gid() -> c::gid_t {
    static V: LazyLock<c::gid_t> = LazyLock::new(uapi::getgid);
    *V
}
