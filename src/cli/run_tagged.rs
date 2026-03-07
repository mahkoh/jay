use {
    crate::{
        cli::GlobalArgs,
        compositor::WAYLAND_DISPLAY,
        tools::tool_client::{Handle, ToolClient, with_tool_client},
        utils::{errorfmt::ErrorFmt, oserror::OsError},
        wire::{jay_acceptor_request, jay_compositor},
    },
    clap::{Args, ValueHint},
    std::{cell::Cell, env, rc::Rc},
    uapi::UstrPtr,
};

#[derive(Args, Debug)]
pub struct RunTaggedArgs {
    /// Specifies a tag to apply to all spawned wayland connections.
    tag: String,
    /// The program to run.
    #[clap(required = true, trailing_var_arg = true, value_hint = ValueHint::CommandWithArguments)]
    pub program: Vec<String>,
}

pub fn main(global: GlobalArgs, run_tagged_args: RunTaggedArgs) {
    with_tool_client(global.log_level, |tc| async move {
        let run_tagged = Rc::new(RunTagged { tc: tc.clone() });
        run_tagged.run(run_tagged_args).await;
    });
}

struct RunTagged {
    tc: Rc<ToolClient>,
}

impl RunTagged {
    async fn run(&self, args: RunTaggedArgs) {
        let tc = &self.tc;
        let comp = tc.jay_compositor().await;
        let req = tc.id();
        tc.send(jay_compositor::GetTaggedAcceptor {
            self_id: comp,
            id: req,
            tag: &args.tag,
        });
        let res = Rc::new(Cell::new(None));
        jay_acceptor_request::Done::handle(&tc, req, res.clone(), |res, ev| {
            res.set(Some(Ok(ev.name.to_owned())));
        });
        jay_acceptor_request::Failed::handle(&tc, req, res.clone(), |res, ev| {
            res.set(Some(Err(ev.msg.to_owned())));
        });
        tc.round_trip().await;
        match res.take().unwrap() {
            Ok(n) => {
                unsafe {
                    env::set_var(WAYLAND_DISPLAY, &n);
                }
                let mut argv = UstrPtr::new();
                for arg in &args.program {
                    argv.push(arg.as_str());
                }
                let program = args.program[0].as_str();
                let res = uapi::execvp(program, &argv).unwrap_err();
                fatal!(
                    "Could not execute `{}`: {}",
                    program,
                    ErrorFmt(OsError::from(res)),
                );
            }
            Err(msg) => {
                fatal!("Could not create acceptor: {}", msg);
            }
        }
    }
}
