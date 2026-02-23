use {
    crate::{
        cli::GlobalArgs,
        tools::tool_client::{Handle, ToolClient, with_tool_client},
        wire::{jay_compositor, jay_open_control_center_request},
    },
    std::{cell::Cell, rc::Rc},
};

pub fn main(global: GlobalArgs) {
    with_tool_client(global.log_level.into(), |tc| async move {
        let cc = ControlCenter { tc: tc.clone() };
        cc.run().await;
    });
}

struct ControlCenter {
    tc: Rc<ToolClient>,
}

impl ControlCenter {
    async fn run(self) {
        let tc = &self.tc;
        let comp = tc.jay_compositor().await;
        let id = tc.id();
        tc.send(jay_compositor::OpenControlCenter { self_id: comp, id });
        let err = Rc::new(Cell::new(None));
        jay_open_control_center_request::Failed::handle(&tc, id, err.clone(), |err, ev| {
            err.set(Some(ev.msg.to_string()));
        });
        tc.round_trip().await;
        if let Some(e) = err.take() {
            fatal!("Could not open the control center: {}", e);
        }
    }
}
