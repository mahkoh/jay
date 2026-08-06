use crate::utils::client_trace::MAX_MESSAGE_WORDS;
use crate::utils::client_trace::WORD_SIZE;
use std::mem::MaybeUninit;

pub fn opt_str_len(val: Option<&str>) -> u32 {
    match val {
        None => 0,
        Some(v) => v.len() as u32 + 1,
    }
}

pub fn opt_str_bytes(s: Option<&str>) -> &[u8] {
    match s {
        None => &[],
        Some(s) => s.as_bytes(),
    }
}

#[allow(clippy::needless_range_loop)]
pub fn write_tail<const N: usize>(
    data: &mut [u32; MAX_MESSAGE_WORDS],
    mut offset: usize,
    val: [&[u8]; N],
) -> Option<usize> {
    let mut required = 0usize;
    if const { size_of::<usize>() == size_of::<u32>() } {
        let mut overflow = false;
        for i in 0..N {
            let o;
            (required, o) = required.overflowing_add((val[i].len() + WORD_SIZE - 1) / WORD_SIZE);
            overflow |= o;
        }
        if overflow {
            return None;
        }
    } else {
        for i in 0..N {
            required = required.wrapping_add((val[i].len() + WORD_SIZE - 1) / WORD_SIZE);
        }
    }
    if data.len() - offset < required {
        return None;
    }
    for i in 0..N {
        let val = val[i];
        let len = val.len();
        unsafe {
            #[allow(deprecated)]
            std::intrinsics::copy_nonoverlapping(
                val.as_ptr(),
                data.as_mut_ptr().add(offset).cast(),
                len,
            );
        }
        offset += (len + WORD_SIZE - 1) / WORD_SIZE;
    }
    Some(offset)
}

#[inline(always)]
pub unsafe fn read_unaligned<T>(p: *const T) -> T {
    unsafe {
        let mut t = MaybeUninit::<T>::uninit();
        #[allow(deprecated)]
        std::intrinsics::copy_nonoverlapping::<u8>(p.cast(), t.as_mut_ptr().cast(), size_of::<T>());
        t.assume_init()
    }
}
