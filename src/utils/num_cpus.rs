use {
    crate::utils::oserror::OsError,
    smallvec::{smallvec_inline, SmallVec},
    uapi::{c, Errno},
};

#[allow(dead_code)]
pub fn num_cpus() -> Result<u32, OsError> {
    let mut buf: SmallVec<[usize; 32]> = smallvec_inline![0; 32];
    loop {
        match uapi::sched_getaffinity(0, &mut buf) {
            Ok(_) => return Ok(count(&buf)),
            Err(Errno(c::EINVAL)) => buf.extend_from_slice(&[0; 32][..]),
            Err(e) => return Err(e.into()),
        }
    }
}

fn count(buf: &[usize]) -> u32 {
    buf.iter().copied().map(|n| n.count_ones()).sum()
}
