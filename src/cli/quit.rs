use crate::cli::GlobalArgs;
use crate::tools::tool_client::ToolClient;
use crate::tools::tool_client::with_tool_client;
use crate::wire::jay_compositor::Quit;
use std::rc::Rc;

pub fn main(global: GlobalArgs) {
    with_tool_client(global.log_level, |tc| async move {
        run(tc).await;
    });
}

async fn run(tc: Rc<ToolClient>) {
    let comp = tc.jay_compositor().await;
    tc.send(Quit { self_id: comp });
    tc.round_trip().await;
}
