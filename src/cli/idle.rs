use {
    crate::{
        cli::{duration::parse_duration, GlobalArgs, IdleArgs, IdleCmd, IdleSetArgs},
        tools::tool_client::{with_tool_client, Handle, ToolClient},
        utils::stack::Stack,
        wire::{jay_compositor, jay_idle, JayIdleId, WlSurfaceId},
    },
    std::{cell::Cell, rc::Rc},
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
        let interval = if args.interval.len() == 1 && args.interval[0] == "disabled" {
            0
        } else {
            parse_duration(&args.interval).as_secs() as u64
        };
        tc.send(jay_idle::SetInterval {
            self_id: idle,
            interval,
        });
        tc.round_trip().await;
    }
}
