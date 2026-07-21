use crate::cli::GlobalArgs;
use crate::tools::tool_client::ToolClient;
use crate::tools::tool_client::with_tool_client;
use crate::wire::jay_compositor::Unlock;
use std::rc::Rc;

pub fn main(_global: GlobalArgs) {
    with_tool_client(|tc| async move {
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
