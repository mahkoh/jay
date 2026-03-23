use {
    crate::{
        cli::{GlobalArgs, json::jsonl},
        tools::tool_client::{Handle, ToolClient, with_tool_client},
        wire::jay_compositor::{GetPid, Pid},
    },
    std::rc::Rc,
};

pub fn main(global: GlobalArgs) {
    with_tool_client(global.log_level, |tc| async move {
        let pid = Rc::new(P { tc: tc.clone() });
        run(&global, pid).await;
    });
}

struct P {
    tc: Rc<ToolClient>,
}

async fn run(global: &GlobalArgs, p: Rc<P>) {
    let tc = &p.tc;
    let comp = tc.jay_compositor().await;
    tc.send(GetPid { self_id: comp });
    let json = global.json;
    Pid::handle(tc, comp, (), move |_, pid| {
        if json {
            jsonl(&pid.pid);
        } else {
            println!("{}", pid.pid);
        }
    });
    tc.round_trip().await;
}
