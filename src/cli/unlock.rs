use {
    crate::{
        cli::GlobalArgs,
        tools::tool_client::{ToolClient, with_tool_client},
        wire::jay_compositor::Unlock,
    },
    std::rc::Rc,
};

pub fn main(global: GlobalArgs) {
    with_tool_client(global.log_level.into(), |tc| async move {
        let logger = Rc::new(Unlocker { tc: tc.clone() });
        run(logger).await;
    });
}

struct Unlocker {
    tc: Rc<ToolClient>,
}

async fn run(log: Rc<Unlocker>) {
    let tc = &log.tc;
    let comp = tc.jay_compositor().await;
    tc.send(Unlock { self_id: comp });
    tc.round_trip().await;
}
