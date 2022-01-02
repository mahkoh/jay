use std::cmp::Ordering;
use std::fmt::{Debug, Formatter};
use std::ops::{Add, Sub};
use std::time::Duration;
use thiserror::Error;
use uapi::c;

#[derive(Debug, Error)]
pub enum TimeError {
    #[error("clock_gettime failed: {0}")]
    ClockGettime(std::io::Error),
}

#[derive(Copy, Clone)]
pub struct Time(pub c::timespec);

impl Debug for Time {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Time")
            .field("tv_sec", &self.0.tv_sec)
            .field("tv_nsec", &self.0.tv_nsec)
            .finish()
    }
}

impl Time {
    pub fn now() -> Result<Time, TimeError> {
        let mut time = uapi::pod_zeroed();
        if let Err(e) = uapi::clock_gettime(c::CLOCK_MONOTONIC, &mut time) {
            return Err(TimeError::ClockGettime(e.into()));
        }
        Ok(Self(time))
    }

    pub fn round_to_ms(self) -> Time {
        if self.0.tv_nsec > 999_000_000 {
            Time(c::timespec {
                tv_sec: self.0.tv_sec + 1,
                tv_nsec: 0,
            })
        } else {
            Time(c::timespec {
                tv_sec: self.0.tv_sec,
                tv_nsec: (self.0.tv_nsec + 999_999) / 1_000_000 * 1_000_000,
            })
        }
    }
}

impl Eq for Time {}

impl PartialEq for Time {
    fn eq(&self, other: &Self) -> bool {
        self.0.tv_sec == other.0.tv_sec && self.0.tv_nsec == other.0.tv_nsec
    }
}

impl Ord for Time {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0
            .tv_sec
            .cmp(&other.0.tv_sec)
            .then_with(|| self.0.tv_nsec.cmp(&other.0.tv_nsec))
    }
}

impl PartialOrd for Time {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Sub<Time> for Time {
    type Output = Duration;

    fn sub(self, rhs: Time) -> Self::Output {
        let sec = self.0.tv_sec - rhs.0.tv_sec;
        let nsec = self.0.tv_nsec - rhs.0.tv_nsec;
        Duration::from_nanos(sec as u64 * 1_000_000_000 + nsec as u64)
    }
}

impl Add<Duration> for Time {
    type Output = Self;

    fn add(mut self, rhs: Duration) -> Self::Output {
        let secs = (rhs.as_nanos() / 1_000_000_000) as c::time_t;
        let nsecs = (rhs.as_nanos() % 1_000_000_000) as c::c_long;
        self.0.tv_sec += secs;
        self.0.tv_nsec += nsecs;
        if self.0.tv_nsec > 999_999_999 {
            self.0.tv_sec += 1;
            self.0.tv_nsec -= 1_000_000_000;
        }
        self
    }
}
