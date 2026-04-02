use {
    crate::{
        io_uring::{IoUring, IoUringError},
        utils::{
            buf::TypedBuf,
            oserror::{OsError, OsErrorExt2},
        },
    },
    std::{cell::RefCell, rc::Rc, time::Duration},
    thiserror::Error,
    uapi::{OwnedFd, c},
};

#[derive(Debug, Error)]
pub enum TimerError {
    #[error("Could not create a timer")]
    CreateTimer(#[source] OsError),
    #[error("Could not read from a timer")]
    TimerReadError(#[source] IoUringError),
    #[error("Could not set a timer")]
    SetTimer(#[source] OsError),
    #[error("The io-uring returned an error")]
    IoUringError(#[from] IoUringError),
}

#[derive(Clone)]
pub struct TimerFd {
    fd: Rc<OwnedFd>,
    buf: Rc<RefCell<TypedBuf<u64>>>,
}

impl TimerFd {
    pub fn new(clock_id: c::c_int) -> Result<Self, TimerError> {
        let fd = uapi::timerfd_create(clock_id, c::TFD_CLOEXEC)
            .map(Rc::new)
            .map_os_err(TimerError::CreateTimer)?;
        Ok(Self {
            fd,
            buf: Rc::new(RefCell::new(TypedBuf::new())),
        })
    }

    #[expect(clippy::await_holding_refcell_ref)]
    pub async fn expired(&self, ring: &IoUring) -> Result<u64, TimerError> {
        let mut buf = self.buf.borrow_mut();
        if let Err(e) = ring.read(&self.fd, buf.buf()).await {
            return Err(TimerError::TimerReadError(e));
        }
        Ok(buf.t())
    }

    pub fn program(
        &self,
        initial: Option<Duration>,
        periodic: Option<Duration>,
    ) -> Result<(), TimerError> {
        let mut timerspec: c::itimerspec = uapi::pod_zeroed();
        if let Some(init) = initial {
            timerspec.it_value.tv_sec = init.as_secs() as _;
            timerspec.it_value.tv_nsec = init.subsec_nanos() as _;
            if let Some(per) = periodic {
                timerspec.it_interval.tv_sec = per.as_secs() as _;
                timerspec.it_interval.tv_nsec = per.subsec_nanos() as _;
            }
        }
        uapi::timerfd_settime(self.fd.raw(), 0, &timerspec).map_os_err(TimerError::SetTimer)?;
        Ok(())
    }
}
