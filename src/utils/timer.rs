pub fn next_expiration(now: u64, last: u64, period: u64) -> u64 {
    let delta = now.saturating_sub(last);
    if delta < period {
        return last.saturating_add(period);
    }
    let Some(delta) = delta.saturating_add(1).checked_next_multiple_of(period) else {
        return u64::MAX;
    };
    last.saturating_add(delta)
}
