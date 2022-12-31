use {
    crate::{
        cli::{GlobalArgs, IdleArgs, IdleCmd, IdleSetArgs},
        tools::tool_client::{with_tool_client, Handle, ToolClient},
        utils::{errorfmt::ErrorFmt, stack::Stack},
        wire::{jay_compositor, jay_idle, JayIdleId, WlSurfaceId},
    },
    std::{cell::Cell, collections::VecDeque, rc::Rc, str::FromStr},
};

pub fn main(global: GlobalArgs, args: IdleArgs) {
    with_tool_client(global.log_level.into(), |tc| async move {
        let idle = Idle { tc: tc.clone() };
        idle.run(args).await;
    });
}

struct Idle {
    tc: Rc<ToolClient>,
}

impl Idle {
    async fn run(self, args: IdleArgs) {
        let tc = &self.tc;
        let comp = tc.jay_compositor().await;
        let idle = tc.id();
        tc.send(jay_compositor::GetIdle {
            self_id: comp,
            id: idle,
        });
        match args.command.unwrap_or_default() {
            IdleCmd::Status => self.status(idle).await,
            IdleCmd::Set(args) => self.set(idle, args).await,
        }
    }

    async fn status(self, idle: JayIdleId) {
        let tc = &self.tc;
        tc.send(jay_idle::GetStatus { self_id: idle });
        let interval = Rc::new(Cell::new(0u64));
        jay_idle::Interval::handle(tc, idle, interval.clone(), |iv, msg| {
            iv.set(msg.interval);
        });
        struct Inhibitor {
            surface: WlSurfaceId,
            _client_id: u64,
            pid: u64,
            comm: String,
        }
        let inhibitors = Rc::new(Stack::default());
        jay_idle::Inhibitor::handle(tc, idle, inhibitors.clone(), |iv, msg| {
            iv.push(Inhibitor {
                surface: msg.surface,
                _client_id: msg.client_id,
                pid: msg.pid,
                comm: msg.comm.to_string(),
            });
        });
        tc.round_trip().await;
        let minutes = interval.get() / 60;
        let seconds = interval.get() % 60;
        print!("Interval:");
        if minutes == 0 && seconds == 0 {
            print!(" disabled");
        } else {
            if minutes > 0 {
                print!(" {} minute", minutes);
                if minutes > 1 {
                    print!("s");
                }
            }
            if seconds > 0 {
                print!(" {} second", seconds);
                if seconds > 1 {
                    print!("s");
                }
            }
        }
        println!();
        let mut inhibitors = inhibitors.take();
        inhibitors.sort_by_key(|i| i.pid);
        inhibitors.sort_by_key(|i| i.surface);
        if inhibitors.len() > 0 {
            println!("Inhibitors:");
            for inhibitor in inhibitors {
                println!(
                    "  {}, surface {}, pid {}",
                    inhibitor.comm, inhibitor.surface, inhibitor.pid
                );
            }
        }
    }

    async fn set(self, idle: JayIdleId, args: IdleSetArgs) {
        let tc = &self.tc;
        let interval;
        if args.interval.len() == 1 && args.interval[0] == "disabled" {
            interval = 0;
        } else {
            let comp = parse_components(&args.interval);
            let mut minutes = None;
            let mut seconds = None;
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
                }
            }
            if pending_num.is_some() {
                fatal!("missing number unit after {}", pending_num.unwrap());
            }
            if minutes.is_none() && seconds.is_none() {
                fatal!("minutes and/or numbers must be specified");
            }
            interval = minutes.unwrap_or(0) * 60 + seconds.unwrap_or(0);
        }
        tc.send(jay_idle::SetInterval {
            self_id: idle,
            interval,
        });
        tc.round_trip().await;
    }
}

#[derive(Debug)]
enum Component {
    Number(u64),
    Minutes(String),
    Seconds(String),
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
