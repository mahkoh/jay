use {
    crate::{
        cli::GlobalArgs,
        tools::tool_client::{ToolClient, with_tool_client},
        wire::jay_compositor::Quit,
    },
    std::rc::Rc,
};

pub fn main(global: GlobalArgs) {
    with_tool_client(global.log_level.into(), |tc| async move {
        run(tc).await;
    });
}

async fn run(tc: Rc<ToolClient>) {
    let comp = tc.jay_compositor().await;
    tc.send(Quit { self_id: comp });
    tc.round_trip().await;
}
