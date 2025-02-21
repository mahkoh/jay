use {
    crate::{
        cli::{GlobalArgs, color::parse_color, duration::parse_duration},
        tools::tool_client::{ToolClient, with_tool_client},
        wire::jay_damage_tracking::{SetVisualizerColor, SetVisualizerDecay, SetVisualizerEnabled},
    },
    clap::{Args, Subcommand},
    std::rc::Rc,
};

#[derive(Args, Debug)]
pub struct DamageTrackingArgs {
    #[clap(subcommand)]
    pub command: DamageTrackingCmd,
}

#[derive(Subcommand, Debug)]
pub enum DamageTrackingCmd {
    /// Visualize damage.
    Show,
    /// Hide damage.
    Hide,
    /// Set the color used for damage visualization.
    SetColor(ColorArgs),
    /// Set the amount of time damage is shown.
    SetDecay(DecayArgs),
}

#[derive(Args, Debug)]
pub struct ColorArgs {
    /// The color to visualize damage.
    ///
    /// Should be specified in one of the following formats:
    ///
    /// * `#rgb`
    /// * `#rgba`
    /// * `#rrggbb`
    /// * `#rrggbbaa`
    pub color: String,
}

#[derive(Args, Debug)]
pub struct DecayArgs {
    /// The interval of inactivity after which to disable the screens.
    ///
    /// Minutes, seconds, and milliseconds can be specified in any of the following formats:
    ///
    /// * 1m
    /// * 1m5s
    /// * 1m 5s
    /// * 1min 5sec
    /// * 1 minute 5 seconds.
    pub duration: Vec<String>,
}

pub fn main(global: GlobalArgs, damage_tracking_args: DamageTrackingArgs) {
    with_tool_client(global.log_level.into(), |tc| async move {
        let damage_tracking = Rc::new(DamageTracking { tc: tc.clone() });
        damage_tracking.run(damage_tracking_args).await;
    });
}

struct DamageTracking {
    tc: Rc<ToolClient>,
}

impl DamageTracking {
    async fn run(&self, args: DamageTrackingArgs) {
        let tc = &self.tc;
        let Some(dt) = tc.jay_damage_tracking().await else {
            fatal!("Compositor does not support damage tracking");
        };
        match args.command {
            DamageTrackingCmd::Show => {
                tc.send(SetVisualizerEnabled {
                    self_id: dt,
                    enabled: 1,
                });
            }
            DamageTrackingCmd::Hide => {
                tc.send(SetVisualizerEnabled {
                    self_id: dt,
                    enabled: 0,
                });
            }
            DamageTrackingCmd::SetColor(c) => {
                let color = parse_color(&c.color);
                tc.send(SetVisualizerColor {
                    self_id: dt,
                    r: color.r,
                    g: color.g,
                    b: color.b,
                    a: color.a,
                });
            }
            DamageTrackingCmd::SetDecay(c) => {
                let duration = parse_duration(&c.duration);
                tc.send(SetVisualizerDecay {
                    self_id: dt,
                    millis: duration.as_millis() as _,
                });
            }
        }
        tc.round_trip().await;
    }
}
