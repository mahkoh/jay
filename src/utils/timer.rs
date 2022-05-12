use {
    crate::{
        io_uring::{IoUring, IoUringError},
        utils::oserror::OsError,
    },
    std::{rc::Rc, time::Duration},
    thiserror::Error,
    uapi::{c, OwnedFd},
};

#[derive(Debug, Error)]
pub enum TimerError {
    #[error("Could not create a timer")]
    CreateTimer(#[source] OsError),
    #[error("Could not read from a timer")]
    TimerReadError(#[source] OsError),
    #[error("Could not set a timer")]
    SetTimer(#[source] OsError),
    #[error("The io-uring returned an error")]
    IoUringError(#[from] IoUringError),
}

#[derive(Clone)]
pub struct TimerFd {
    fd: Rc<OwnedFd>,
}

impl TimerFd {
    pub fn new(clock_id: c::c_int) -> Result<Self, TimerError> {
        let fd = match uapi::timerfd_create(clock_id, c::TFD_CLOEXEC | c::TFD_NONBLOCK) {
            Ok(fd) => Rc::new(fd),
            Err(e) => return Err(TimerError::CreateTimer(e.into())),
        };
        Ok(Self { fd })
    }

    pub async fn expired(&self, ring: &IoUring) -> Result<u64, TimerError> {
        ring.readable(&self.fd).await?;
        let mut buf = 0u64;
        if let Err(e) = uapi::read(self.fd.raw(), &mut buf) {
            return Err(TimerError::TimerReadError(e.into()));
        }
        Ok(buf)
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
        if let Err(e) = uapi::timerfd_settime(self.fd.raw(), 0, &timerspec) {
            return Err(TimerError::SetTimer(e.into()));
        }
        Ok(())
    }
}
