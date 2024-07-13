use {
    crate::utils::errorfmt::ErrorFmt,
    std::{collections::VecDeque, str::FromStr, time::Duration},
};

#[derive(Debug)]
enum Component {
    Number(u64),
    Minutes(String),
    Seconds(String),
    Milliseconds(String),
}

pub fn parse_duration(args: &[String]) -> Duration {
    let comp = parse_components(args);
    let mut minutes = None;
    let mut seconds = None;
    let mut milliseconds = None;
    let mut pending_num = None;
    for comp in comp {
        match comp {
            Component::Number(_) if pending_num.is_some() => {
                fatal!("missing number unit after {}", pending_num.unwrap())
            }
            Component::Number(n) => pending_num = Some(n),

            Component::Minutes(n) if pending_num.is_none() => {
                fatal!("`{}` must be preceded by a number", n)
            }
            Component::Minutes(_) if minutes.is_some() => {
                fatal!("minutes specified multiple times")
            }
            Component::Minutes(_) => minutes = pending_num.take(),

            Component::Seconds(n) if pending_num.is_none() => {
                fatal!("`{}` must be preceded by a number", n)
            }
            Component::Seconds(_) if seconds.is_some() => {
                fatal!("seconds specified multiple times")
            }
            Component::Seconds(_) => seconds = pending_num.take(),
            Component::Milliseconds(n) if pending_num.is_none() => {
                fatal!("`{}` must be preceded by a number", n)
            }
            Component::Milliseconds(_) if milliseconds.is_some() => {
                fatal!("milliseconds specified multiple times")
            }
            Component::Milliseconds(_) => milliseconds = pending_num.take(),
        }
    }
    if pending_num.is_some() {
        fatal!("missing number unit after {}", pending_num.unwrap());
    }
    if minutes.is_none() && seconds.is_none() && milliseconds.is_none() {
        fatal!("duration must be specified");
    }
    let mut ms = minutes.unwrap_or(0) as u128 * 60 * 1000
        + seconds.unwrap_or(0) as u128 * 1000
        + milliseconds.unwrap_or(0) as u128;
    if ms > u64::MAX as u128 {
        ms = u64::MAX as u128;
    }
    Duration::from_millis(ms as u64)
}

fn parse_components(args: &[String]) -> Vec<Component> {
    let mut args = VecDeque::from_iter(args.iter().map(|s| s.to_ascii_lowercase()));
    let mut res = vec![];
    while let Some(arg) = args.pop_front() {
        if arg.is_empty() {
            continue;
        }
        let mut arg = &arg[..];
        if is_num(arg.as_bytes()[0]) {
            if let Some(pos) = arg.as_bytes().iter().position(|&a| !is_num(a)) {
                args.push_front(arg[pos..].to_string());
                arg = &arg[..pos];
            }
            match u64::from_str(arg) {
                Ok(n) => res.push(Component::Number(n)),
                Err(e) => fatal!("Could not parse `{}` as a number: {}", arg, ErrorFmt(e)),
            }
        } else {
            if let Some(pos) = arg.as_bytes().iter().position(|&a| is_num(a)) {
                args.push_front(arg[pos..].to_string());
                arg = &arg[..pos];
            }
            let comp = match arg {
                "minutes" | "minute" | "min" | "m" => Component::Minutes(arg.to_string()),
                "seconds" | "second" | "sec" | "s" => Component::Seconds(arg.to_string()),
                "milliseconds" | "millisecond" | "ms" => Component::Milliseconds(arg.to_string()),
                _ => fatal!("Could not parse `{}`", arg),
            };
            res.push(comp);
        }
    }
    res
}

fn is_num(b: u8) -> bool {
    matches!(b, b'0'..=b'9')
}
