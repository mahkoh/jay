use {
    crate::{
        cli::GlobalArgs,
        tools::tool_client::{Handle, ToolClient, with_tool_client},
        wire::{JayXwaylandId, jay_compositor, jay_xwayland},
    },
    clap::{Args, Subcommand, ValueEnum},
    jay_config::xwayland::XScalingMode,
    std::{cell::Cell, rc::Rc},
};

#[derive(Args, Debug)]
pub struct XwaylandArgs {
    #[clap(subcommand)]
    pub command: Option<XwaylandCmd>,
}

#[derive(Subcommand, Debug, Default)]
pub enum XwaylandCmd {
    /// Print the Xwayland status.
    #[default]
    Status,
    /// Set the Xwayland scaling mode.
    SetScalingMode(SetScalingModeArgs),
}

#[derive(Args, Debug)]
pub struct SetScalingModeArgs {
    #[clap(value_enum)]
    pub mode: CliScalingMode,
}

#[derive(ValueEnum, Debug, Copy, Clone, Hash, PartialEq)]
pub enum CliScalingMode {
    /// The default mode.
    Default,
    /// Windows are rendered at the highest integer scale and then downscaled.
    Downscaled,
}

pub fn main(global: GlobalArgs, args: XwaylandArgs) {
    with_tool_client(global.log_level.into(), |tc| async move {
        let xwayland = Xwayland { tc: tc.clone() };
        xwayland.run(args).await;
    });
}

struct Xwayland {
    tc: Rc<ToolClient>,
}

impl Xwayland {
    async fn run(self, args: XwaylandArgs) {
        let tc = &self.tc;
        let comp = tc.jay_compositor().await;
        let xwayland = tc.id();
        tc.send(jay_compositor::GetXwayland {
            self_id: comp,
            id: xwayland,
        });
        match args.command.unwrap_or_default() {
            XwaylandCmd::Status => self.status(xwayland).await,
            XwaylandCmd::SetScalingMode(args) => self.set_scaling_mode(xwayland, args).await,
        }
    }

    async fn status(self, xwayland: JayXwaylandId) {
        let tc = &self.tc;
        tc.send(jay_xwayland::GetScaling { self_id: xwayland });
        let mode = Rc::new(Cell::new(0));
        let scale = Rc::new(Cell::new(None));
        jay_xwayland::ScalingMode::handle(tc, xwayland, mode.clone(), |iv, msg| {
            iv.set(msg.mode);
        });
        jay_xwayland::ImpliedScale::handle(tc, xwayland, scale.clone(), |iv, msg| {
            iv.set(Some(msg.scale));
        });
        tc.round_trip().await;
        let mode_str;
        let mode = match XScalingMode(mode.get()) {
            XScalingMode::DEFAULT => "default",
            XScalingMode::DOWNSCALED => "downscaled",
            o => {
                mode_str = format!("unknown ({})", o.0);
                &mode_str
            }
        };
        println!("scaling mode: {}", mode);
        if let Some(scale) = scale.get() {
            println!("implied scale: {}", scale);
        }
    }

    async fn set_scaling_mode(self, xwayland: JayXwaylandId, args: SetScalingModeArgs) {
        let tc = &self.tc;
        let mode = match args.mode {
            CliScalingMode::Default => XScalingMode::DEFAULT,
            CliScalingMode::Downscaled => XScalingMode::DOWNSCALED,
        };
        tc.send(jay_xwayland::SetScalingMode {
            self_id: xwayland,
            mode: mode.0,
        });
        tc.round_trip().await;
    }
}
