use {
    crate::{cli::GlobalArgs, tools::tool_client::ToolClient, wire::jay_compositor::Unlock},
    std::rc::Rc,
};

pub fn main(global: GlobalArgs) {
    let tc = ToolClient::new(global.log_level.into());
    let logger = Rc::new(Unlocker { tc: tc.clone() });
    tc.run(run(logger));
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
