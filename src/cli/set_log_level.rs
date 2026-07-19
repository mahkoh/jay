use crate::cli::GlobalArgs;
use crate::cli::SetLogArgs;
use crate::tools::tool_client::ToolClient;
use crate::tools::tool_client::with_tool_client;
use crate::wire::jay_compositor::SetLogLevel;
use linearize::Linearize;
use std::rc::Rc;

pub fn main(global: GlobalArgs, args: SetLogArgs) {
    with_tool_client(global.log_level, |tc| async move {
        let logger = Rc::new(Log {
            tc: tc.clone(),
            args,
        });
        run(logger).await;
    });
}

struct Log {
    tc: Rc<ToolClient>,
    args: SetLogArgs,
}

async fn run(log: Rc<Log>) {
    let tc = &log.tc;
    let comp = tc.jay_compositor().await;
    tc.send(SetLogLevel {
        self_id: comp,
        level: log.args.level.linearize() as u32,
    });
    tc.round_trip().await;
}
