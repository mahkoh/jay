use {
    crate::{cli::GlobalArgs, tools::tool_client::ToolClient, wire::jay_compositor::Quit},
    std::rc::Rc,
};

pub fn main(global: GlobalArgs) {
    let tc = ToolClient::new(global.log_level.into());
    tc.run(run(tc.clone()));
}

async fn run(tc: Rc<ToolClient>) {
    let comp = tc.jay_compositor().await;
    tc.send(Quit { self_id: comp });
    tc.round_trip().await;
}
