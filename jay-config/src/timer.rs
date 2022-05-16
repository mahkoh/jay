//! Timers for one-time or repeated actions.

use {
    bincode::{Decode, Encode},
    std::time::{Duration, SystemTime, UNIX_EPOCH},
};

/// A timer.
#[derive(Encode, Decode, Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Timer(pub u64);

/// Creates a new timer or returns an existing one.
///
/// Timers are identified by their name and their lifetime is bound by the lifetime of
/// the configuration. Reloading the configuration destroys all existing timers.
///
/// Within the same configuration, calling this function multiple times with the same name
/// will return the same timer.
///
/// Timers can be deleted by calling `remove`. At that point all existing references to
/// the timer become invalid and `get_timer` will return a new timer.
pub fn get_timer(name: &str) -> Timer {
    get!(Timer(0)).get_timer(name)
}

impl Timer {
    /// Programs the timer to fire once.
    pub fn once(self, initial: Duration) {
        get!().program_timer(self, Some(initial), None);
    }

    /// Programs the timer to fire repeatedly.
    ///
    /// `initial` is the period after which the timer expires for the first time.
    pub fn repeated(self, initial: Duration, period: Duration) {
        get!().program_timer(self, Some(initial), Some(period));
    }

    /// Cancels the timer.
    ///
    /// The timer remains valid but will never expire. It can be reprogrammed by calling
    /// `once` or `repeated`.
    pub fn cancel(self) {
        get!().program_timer(self, None, None);
    }

    /// Removes the time.
    ///
    /// This reference to the timer becomes invalid as do all other existing references.
    /// A new timer with the same name can be created by calling `get_timer`.
    pub fn remove(self) {
        get!().remove_timer(self);
    }

    /// Sets the function to be executed when the timer expires.
    pub fn on_tick<F: Fn() + 'static>(self, f: F) {
        get!().on_timer_tick(self, f);
    }
}

/// Returns the duration until the wall clock is a multiple of `duration`.
///
/// # Example
///
/// Execute a timer every time the wall clock becomes a multiple of 5 seconds:
///
/// ```rust,ignore
/// let period = Duration::from_secs(5);
/// let timer = get_timer("status_timer");
/// timer.repeated(
///     duration_until_wall_clock_is_multiple_of(period),
///     period,
/// );
/// timer.on_tick(|| todo!());
/// ```
pub fn duration_until_wall_clock_is_multiple_of(duration: Duration) -> Duration {
    let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(n) => n,
        _ => return Duration::from_secs(0),
    };
    let now = now.as_nanos();
    let duration = duration.as_nanos();
    if duration == 0 {
        return Duration::from_secs(0);
    }
    let nanos = duration - now % duration;
    if nanos == duration {
        Duration::from_secs(0)
    } else {
        Duration::from_nanos(nanos as _)
    }
}
