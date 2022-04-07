use {
    std::{
        mem,
        rc::Rc,
        sync::{Arc, Condvar, Mutex},
    },
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
            let mut lock = slf2.fds.lock().unwrap();
            loop {
                mem::swap(&mut *lock, &mut fds);
                if fds.len() > 0 {
                    drop(lock);
                    fds.clear();
                    lock = slf2.fds.lock().unwrap();
                } else {
                    lock = slf2.cv.wait(lock).unwrap();
                }
            }
        });
        slf
    }

    pub fn close(&self, fd: Rc<OwnedFd>) {
        match Rc::try_unwrap(fd) {
            Ok(fd) => {
                self.fds.lock().unwrap().push(fd);
                self.cv.notify_all();
            }
            Err(_e) => {
                log::warn!("Could not close file descriptor in separate thread. There are still references.");
            }
        }
    }
}
