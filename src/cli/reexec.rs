use {
    crate::{
        cli::GlobalArgs,
        tools::tool_client::{Handle, ToolClient, with_tool_client},
        utils::errorfmt::ErrorFmt,
        wire::{jay_compositor, jay_reexec},
    },
    clap::Args,
    std::rc::Rc,
};

#[derive(Args, Debug)]
pub struct ReexecArgs {
    /// The path to the new executable.
    ///
    /// If this is not given, uses the path of **this** executable instead.
    path: Option<String>,
    /// The arguments to pass to the new executable.
    args: Vec<String>,
}

pub fn main(global: GlobalArgs, reexec_args: ReexecArgs) {
    with_tool_client(global.log_level.into(), |tc| async move {
        let rexec = Rc::new(Reexec { tc: tc.clone() });
        rexec.run(reexec_args).await;
    });
}

struct Reexec {
    tc: Rc<ToolClient>,
}

impl Reexec {
    async fn run(&self, args: ReexecArgs) {
        let tc = &self.tc;
        let comp = tc.jay_compositor().await;
        let reexec = tc.id();
        tc.send(jay_compositor::Reexec {
            self_id: comp,
            id: reexec,
        });
        if let Some(path) = &args.path {
            for arg in &args.args {
                tc.send(jay_reexec::Arg {
                    self_id: reexec,
                    arg,
                });
            }
            tc.send(jay_reexec::Exec {
                self_id: reexec,
                path,
            });
        } else {
            let exe = match std::env::current_exe() {
                Ok(e) => e,
                Err(e) => {
                    log::error!("Could not determine the executable path: {}", ErrorFmt(e));
                    std::process::exit(1);
                }
            };
            let Some(exe) = exe.to_str() else {
                log::error!("Executable path is not a string: {:?}", exe);
                std::process::exit(1);
            };
            tc.send(jay_reexec::Arg {
                self_id: reexec,
                arg: "run",
            });
            tc.send(jay_reexec::Exec {
                self_id: reexec,
                path: exe,
            });
        }
        jay_reexec::Failed::handle(tc, reexec, (), |_, msg| {
            log::error!("Exec failed: {}", msg.msg);
            std::process::exit(1);
        });
        tc.round_trip().await;
    }
}
