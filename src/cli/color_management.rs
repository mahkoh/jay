use {
    crate::{
        cli::GlobalArgs,
        tools::tool_client::{Handle, ToolClient, with_tool_client},
        wire::{JayColorManagementId, jay_color_management, jay_compositor},
    },
    clap::{Args, Subcommand},
    std::{cell::Cell, rc::Rc},
};

#[derive(Args, Debug)]
pub struct ColorManagementArgs {
    #[clap(subcommand)]
    pub command: Option<ColorManagementCmd>,
}

#[derive(Subcommand, Debug, Default)]
pub enum ColorManagementCmd {
    /// Print the color-management status.
    #[default]
    Status,
    /// Enable the color-management protocol.
    Enable,
    /// Disable the color-management protocol.
    Disable,
}

pub fn main(global: GlobalArgs, args: ColorManagementArgs) {
    with_tool_client(global.log_level.into(), |tc| async move {
        let cm = ColorManagement { tc: tc.clone() };
        cm.run(args).await;
    });
}

struct ColorManagement {
    tc: Rc<ToolClient>,
}

impl ColorManagement {
    async fn run(self, args: ColorManagementArgs) {
        let tc = &self.tc;
        let comp = tc.jay_compositor().await;
        let id = tc.id();
        tc.send(jay_compositor::GetColorManagement { self_id: comp, id });
        match args.command.unwrap_or_default() {
            ColorManagementCmd::Status => self.status(id).await,
            ColorManagementCmd::Enable => self.set_enabled(id, true).await,
            ColorManagementCmd::Disable => self.set_enabled(id, false).await,
        }
    }

    async fn status(self, id: JayColorManagementId) {
        let tc = &self.tc;
        tc.send(jay_color_management::Get { self_id: id });
        let enabled = Rc::new(Cell::new(false));
        jay_color_management::Enabled::handle(tc, id, enabled.clone(), |iv, msg| {
            iv.set(msg.enabled != 0);
        });
        let available = Rc::new(Cell::new(false));
        jay_color_management::Available::handle(tc, id, available.clone(), |iv, msg| {
            iv.set(msg.available != 0);
        });
        tc.round_trip().await;
        if enabled.get() {
            print!("Enabled");
            if !available.get() {
                print!(" (Unavailable)");
            }
            println!();
        } else {
            println!("Disabled");
        }
    }

    async fn set_enabled(self, id: JayColorManagementId, enabled: bool) {
        let tc = &self.tc;
        tc.send(jay_color_management::SetEnabled {
            self_id: id,
            enabled: enabled as _,
        });
        tc.round_trip().await;
    }
}
