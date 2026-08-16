use std::sync::LazyLock;
use uapi::c;

#[expect(unused)]
pub fn uid() -> c::uid_t {
    static V: LazyLock<c::uid_t> = LazyLock::new(uapi::getuid);
    *V
}

#[expect(unused)]
pub fn gid() -> c::gid_t {
    static V: LazyLock<c::gid_t> = LazyLock::new(uapi::getgid);
    *V
}
