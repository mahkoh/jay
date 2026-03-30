use {std::sync::LazyLock, uapi::c};

static PAGE_SIZE: LazyLock<usize> =
    LazyLock::new(|| uapi::sysconf(c::_SC_PAGESIZE).unwrap_or(4096) as _);

pub fn page_size() -> usize {
    *PAGE_SIZE
}
