use {
    std::{
        cmp::Ordering,
        fmt::{Debug, Formatter},
        ops::{Add, Sub},
        time::Duration,
    },
    uapi::c,
};

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
    pub fn now_unchecked() -> Time {
        let mut time = uapi::pod_zeroed();
        let _ = uapi::clock_gettime(c::CLOCK_MONOTONIC, &mut time);
        Self(time)
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

    pub fn nsec(self) -> u64 {
        let sec = self.0.tv_sec as u64 * 1_000_000_000;
        let nsec = self.0.tv_nsec as u64;
        sec + nsec
    }

    pub fn usec(self) -> u64 {
        let sec = self.0.tv_sec as u64 * 1_000_000;
        let nsec = self.0.tv_nsec as u64 / 1_000;
        sec + nsec
    }

    pub fn msec(self) -> u64 {
        let sec = self.0.tv_sec as u64 * 1_000;
        let nsec = self.0.tv_nsec as u64 / 1_000_000;
        sec + nsec
    }

    pub fn elapsed(self) -> Duration {
        let now = Self::now_unchecked();
        now - self
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
        Duration::from_nanos((sec as i64 * 1_000_000_000 + nsec as i64) as u64)
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

pub fn usec_to_msec(usec: u64) -> u32 {
    (usec / 1000) as u32
}
