use {
    crate::{
        cli::GlobalArgs,
        tools::tool_client::{Handle, ToolClient, with_tool_client},
        wire::jay_compositor::{GetPid, Pid},
    },
    std::rc::Rc,
};

pub fn main(global: GlobalArgs) {
    with_tool_client(global.log_level, |tc| async move {
        let pid = Rc::new(P { tc: tc.clone() });
        run(pid).await;
    });
}

struct P {
    tc: Rc<ToolClient>,
}

async fn run(p: Rc<P>) {
    let tc = &p.tc;
    let comp = tc.jay_compositor().await;
    tc.send(GetPid { self_id: comp });
    Pid::handle(tc, comp, (), |_, pid| {
        println!("{}", pid.pid);
    });
    tc.round_trip().await;
}
