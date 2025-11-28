use {
    crate::{
        cli::{GlobalArgs, IdleArgs, duration::parse_duration},
        tools::tool_client::{Handle, ToolClient, with_tool_client},
        utils::{debug_fn::debug_fn, stack::Stack},
        wire::{JayIdleId, WlSurfaceId, jay_compositor, jay_idle},
    },
    clap::{Args, Subcommand},
    std::{cell::Cell, rc::Rc},
};

#[derive(Subcommand, Debug, Default)]
pub enum IdleCmd {
    /// Print the idle status.
    #[default]
    Status,
    /// Set the idle interval.
    Set(IdleSetArgs),
    /// Set the idle grace period.
    SetGracePeriod(IdleSetGracePeriodArgs),
}

#[derive(Args, Debug)]
pub struct IdleSetArgs {
    /// The interval of inactivity after which to disable the screens.
    ///
    /// This can be either a number in minutes and seconds or the keyword `disabled` to
    /// disable the screensaver.
    ///
    /// Minutes and seconds can be specified in any of the following formats:
    ///
    /// * 1m
    /// * 1m5s
    /// * 1m 5s
    /// * 1min 5sec
    /// * 1 minute 5 seconds
    #[clap(verbatim_doc_comment, required = true)]
    pub interval: Vec<String>,
}

#[derive(Args, Debug)]
pub struct IdleSetGracePeriodArgs {
    /// The grace period after the idle timeout expires.
    ///
    /// During this period, after the idle timeout expires, the screen only goes black
    /// but is not yet disabled or locked.
    ///
    /// This uses the same formatting options as the idle timeout itself.
    #[clap(verbatim_doc_comment, required = true)]
    pub period: Vec<String>,
}

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
            IdleCmd::SetGracePeriod(args) => self.set_grace_period(idle, args).await,
        }
    }

    async fn status(self, idle: JayIdleId) {
        let tc = &self.tc;
        tc.send(jay_idle::GetStatus { self_id: idle });
        let timeout = Rc::new(Cell::new(0u64));
        jay_idle::Interval::handle(tc, idle, timeout.clone(), |iv, msg| {
            iv.set(msg.interval);
        });
        let grace = Rc::new(Cell::new(0u64));
        jay_idle::GracePeriod::handle(tc, idle, grace.clone(), |iv, msg| {
            iv.set(msg.period);
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
        let interval = |iv: u64| {
            debug_fn(move |f| {
                let minutes = iv / 60;
                let seconds = iv % 60;
                if minutes == 0 && seconds == 0 {
                    write!(f, " disabled")?;
                } else {
                    if minutes > 0 {
                        write!(f, " {} minute", minutes)?;
                        if minutes > 1 {
                            write!(f, "s")?;
                        }
                    }
                    if seconds > 0 {
                        write!(f, " {} second", seconds)?;
                        if seconds > 1 {
                            write!(f, "s")?;
                        }
                    }
                }
                Ok(())
            })
        };
        println!("Interval:{}", interval(timeout.get()));
        println!("Grace period:{}", interval(grace.get()));
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
        tc.send(jay_idle::SetInterval {
            self_id: idle,
            interval: parse_idle_time(&args.interval),
        });
        tc.round_trip().await;
    }

    async fn set_grace_period(self, idle: JayIdleId, args: IdleSetGracePeriodArgs) {
        let tc = &self.tc;
        tc.send(jay_idle::SetGracePeriod {
            self_id: idle,
            period: parse_idle_time(&args.period),
        });
        tc.round_trip().await;
    }
}

fn parse_idle_time(time: &[String]) -> u64 {
    if time.len() == 1 && time[0] == "disabled" {
        0
    } else {
        parse_duration(time).as_secs() as u64
    }
}
