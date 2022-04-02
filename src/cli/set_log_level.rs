use crate::cli::{GlobalArgs, SetLogArgs};
use crate::tools::tool_client::ToolClient;
use crate::wire::jay_compositor::SetLogLevel;
use std::rc::Rc;

pub fn main(global: GlobalArgs, args: SetLogArgs) {
    let tc = ToolClient::new(global.log_level.into());
    let logger = Rc::new(Log {
        tc: tc.clone(),
        args,
    });
    tc.run(run(logger));
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
        level: log.args.level as u32,
    });
    tc.round_trip().await;
}
