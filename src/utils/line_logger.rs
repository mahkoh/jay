use {
    crate::{
        io_uring::{IoUring, IoUringError},
        utils::{buf::Buf, vecdeque_ext::VecDequeExt},
    },
    isnt::std_1::collections::IsntVecDequeExt,
    std::{collections::VecDeque, rc::Rc},
    uapi::OwnedFd,
};

pub async fn log_lines(
    ring: &IoUring,
    fd: &Rc<OwnedFd>,
    mut f: impl FnMut(&[u8], &[u8]),
) -> Result<(), IoUringError> {
    let mut buf = VecDeque::<u8>::new();
    let mut buf2 = Buf::new(1024);
    let mut done = false;
    while !done {
        let n = ring.read(fd, buf2.clone()).await?;
        buf.extend(&buf2[..n]);
        if n == 0 {
            done = true;
        }
        while let Some(pos) = buf.iter().position(|b| b == &b'\n') {
            let (left, right) = buf.get_slices(..pos);
            f(left, right);
            buf.drain(..=pos);
        }
    }
    if buf.is_not_empty() {
        let (left, right) = buf.as_slices();
        f(left, right);
    }
    Ok(())
}
