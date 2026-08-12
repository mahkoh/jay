include!(concat!(env!("OUT_DIR"), "/time_lt.rs"));

pub struct SplitUsec {
    pub hour: &'static str,
    pub minute: &'static str,
    pub second: &'static str,
    pub usec_hi: &'static str,
    pub usec_mi: &'static str,
    pub usec_lo: &'static str,
}

const USEC_PER_SECOND: u32 = 1_000_000;
const USEC_PER_MINUTE: u32 = 60 * USEC_PER_SECOND;
const USEC_PER_HOUR: u64 = 60 * USEC_PER_MINUTE as u64;
const USEC_PER_DAY: u64 = 24 * USEC_PER_HOUR;

pub fn split_usec(time: u64) -> SplitUsec {
    let days = time / USEC_PER_DAY;
    let sub_day_usec = time - days * USEC_PER_DAY;
    let hours = sub_day_usec / USEC_PER_HOUR;
    let sub_hour_usec = (sub_day_usec - hours * USEC_PER_HOUR) as u32;
    let minutes = sub_hour_usec / USEC_PER_MINUTE;
    let sub_minute_usec = sub_hour_usec - minutes * USEC_PER_MINUTE;
    let seconds = sub_minute_usec / USEC_PER_SECOND;
    let sub_second_usec = sub_minute_usec - seconds * USEC_PER_SECOND;
    let usec_hi = sub_second_usec / 1_00_00;
    let rem = sub_second_usec - usec_hi * 1_00_00;
    let usec_mi = rem / 1_00;
    let usec_lo = rem - usec_mi * 1_00;
    unsafe {
        SplitUsec {
            hour: LT.get_unchecked(hours as usize).get(),
            minute: LT.get_unchecked(minutes as usize).get(),
            second: LT.get_unchecked(seconds as usize).get(),
            usec_hi: LT.get_unchecked(usec_hi as usize).get(),
            usec_mi: LT.get_unchecked(usec_mi as usize).get(),
            usec_lo: LT.get_unchecked(usec_lo as usize).get(),
        }
    }
}
