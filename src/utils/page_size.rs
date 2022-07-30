use {once_cell::sync::Lazy, uapi::c};

static PAGE_SIZE: Lazy<usize> = Lazy::new(|| uapi::sysconf(c::_SC_PAGESIZE).unwrap_or(4096) as _);

pub fn page_size() -> usize {
    *PAGE_SIZE
}
