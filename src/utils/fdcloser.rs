use {
    parking_lot::{Condvar, Mutex},
    std::{mem, rc::Rc, sync::Arc},
    uapi::OwnedFd,
};

pub struct FdCloser {
    fds: Mutex<Vec<OwnedFd>>,
    cv: Condvar,
}

impl FdCloser {
    pub fn new() -> Arc<Self> {
        let slf = Arc::new(Self {
            fds: Mutex::new(Vec::new()),
            cv: Condvar::new(),
        });
        let slf2 = slf.clone();
        std::thread::spawn(move || {
            let mut fds = vec![];
            let mut lock = slf2.fds.lock();
            loop {
                mem::swap(&mut *lock, &mut fds);
                if fds.len() > 0 {
                    drop(lock);
                    fds.clear();
                    lock = slf2.fds.lock();
                } else {
                    slf2.cv.wait(&mut lock);
                }
            }
        });
        slf
    }

    pub fn close(&self, fd: Rc<OwnedFd>) {
        match Rc::try_unwrap(fd) {
            Ok(fd) => {
                self.fds.lock().push(fd);
                self.cv.notify_all();
            }
            Err(_e) => {
                log::warn!("Could not close file descriptor in separate thread. There are still references.");
            }
        }
    }
}
