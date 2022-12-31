use {
    crate::{
        cli::{GlobalArgs, SetLogArgs},
        tools::tool_client::{with_tool_client, ToolClient},
        wire::jay_compositor::SetLogLevel,
    },
    std::rc::Rc,
};

pub fn main(global: GlobalArgs, args: SetLogArgs) {
    with_tool_client(global.log_level.into(), |tc| async move {
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
        level: log.args.level as u32,
    });
    tc.round_trip().await;
}
