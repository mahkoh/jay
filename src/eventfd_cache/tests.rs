use {
    crate::{
        async_engine::AsyncEngine, eventfd_cache::EventfdCache, io_uring::IoUring, utils::array,
    },
    std::{rc::Rc, slice},
    uapi::c,
};

#[test]
fn test() {
    let eng = AsyncEngine::new();
    let ring = IoUring::new(&eng, 32).unwrap();
    let cache = Rc::new(EventfdCache::new(&ring, &eng));
    const TOTAL: usize = 5;
    let signaled = 3;
    let fd1: [_; TOTAL] = array::from_fn(|_| cache.acquire().unwrap());
    let fd2: [_; TOTAL] = array::from_fn(|_| cache.acquire().unwrap());
    for fd in fd1.iter().chain(fd2.iter()) {
        uapi::eventfd_write(fd.fd.raw(), 1).unwrap();
        let mut poll = c::pollfd {
            fd: fd.fd.raw(),
            events: c::POLLIN,
            revents: 0,
        };
        uapi::poll(slice::from_mut(&mut poll), 0).unwrap();
        assert_eq!(poll.revents, c::POLLIN);
    }
    assert_eq!(cache.inner.fds.len(), 0);
    let ring2 = ring.clone();
    let cache2 = cache.clone();
    let _fut1 = eng.spawn("", async move {
        for i in 0..signaled {
            fd1[i].signaled().await.unwrap();
        }
        drop(fd1);
        let debouncer = ring2.debouncer(0);
        while cache2.inner.fds.len() != signaled {
            debouncer.debounce().await;
        }
        for i in 0..signaled {
            fd2[i].signaled().await.unwrap();
        }
        drop(fd2);
        while cache2.inner.fds.len() != 2 * signaled {
            debouncer.debounce().await;
        }
        ring2.stop();
    });
    let now_nsec = eng.now().nsec();
    let ring2 = ring.clone();
    let _fut2 = eng.spawn("", async move {
        ring2.timeout(now_nsec + 1_000_000_000).await.unwrap();
        ring2.stop();
    });
    ring.run().unwrap();
    assert_eq!(cache.inner.fds.len(), 2 * signaled);
    for fd in cache.inner.fds.take() {
        let mut poll = c::pollfd {
            fd: fd.raw(),
            events: c::POLLIN,
            revents: 0,
        };
        uapi::poll(slice::from_mut(&mut poll), 0).unwrap();
        assert_eq!(poll.revents, 0);
    }
}
