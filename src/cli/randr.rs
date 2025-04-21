use {
    crate::{
        backend::{BackendColorSpace, BackendTransferFunction},
        cli::GlobalArgs,
        format::{Format, XRGB8888},
        scale::Scale,
        tools::tool_client::{Handle, ToolClient, with_tool_client},
        utils::{errorfmt::ErrorFmt, transform_ext::TransformExt},
        wire::{JayRandrId, jay_compositor, jay_randr},
    },
    clap::{
        Args, Subcommand, ValueEnum,
        builder::{PossibleValue, PossibleValuesParser},
    },
    isnt::std_1::vec::IsntVecExt,
    jay_config::video::{TearingMode, Transform, VrrMode},
    linearize::LinearizeExt,
    std::{
        cell::RefCell,
        fmt::{Display, Formatter},
        rc::Rc,
        str::FromStr,
        time::Duration,
    },
    thiserror::Error,
};

#[derive(Args, Debug)]
pub struct RandrArgs {
    #[clap(subcommand)]
    pub command: Option<RandrCmd>,
}

#[derive(Subcommand, Debug)]
pub enum RandrCmd {
    /// Show the current settings.
    Show(ShowArgs),
    /// Modify the settings of a graphics card.
    Card(CardArgs),
    /// Modify the settings of an output.
    Output(OutputArgs),
}

impl Default for RandrCmd {
    fn default() -> Self {
        Self::Show(Default::default())
    }
}

#[derive(Args, Debug, Default)]
pub struct ShowArgs {
    /// Show all available modes.
    #[arg(long)]
    pub modes: bool,
    /// Show all available formats.
    #[arg(long)]
    pub formats: bool,
}

#[derive(Args, Debug)]
pub struct CardArgs {
    /// The card to modify, e.g. card0.
    pub card: String,
    #[clap(subcommand)]
    pub command: CardCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum CardCommand {
    /// Make this device the primary device.
    Primary,
    /// Modify the graphics API used by the card.
    Api(ApiArgs),
    /// Modify the direct scanout setting of the card.
    DirectScanout(DirectScanoutArgs),
    /// Modify timing settings of the card.
    Timing(TimingArgs),
}

#[derive(Args, Debug, Clone)]
pub struct TimingArgs {
    #[clap(subcommand)]
    pub cmd: TimingCmd,
}

#[derive(Subcommand, Debug, Clone)]
pub enum TimingCmd {
    /// Sets the margin to use for page flips.
    ///
    /// This is duration between the compositor initiating a page flip and the output's
    /// vblank event. This determines the minimum input latency. The default is 1.5 ms.
    ///
    /// Note that if the margin is too small, the compositor will dynamically increase it.
    SetFlipMargin(SetFlipMarginArgs),
}

#[derive(Args, Debug, Clone)]
pub struct SetFlipMarginArgs {
    /// The margin in milliseconds.
    pub margin_ms: f64,
}

#[derive(Args, Debug, Clone)]
pub struct ApiArgs {
    #[clap(subcommand)]
    pub cmd: ApiCmd,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ApiCmd {
    /// Use OpenGL for rendering in this card.
    #[clap(name = "opengl")]
    OpenGl,
    /// Use Vulkan for rendering in this card.
    #[clap(name = "vulkan")]
    Vulkan,
}

#[derive(Args, Debug, Clone)]
pub struct DirectScanoutArgs {
    #[clap(subcommand)]
    pub cmd: DirectScanoutCmd,
}

#[derive(Subcommand, Debug, Clone)]
pub enum DirectScanoutCmd {
    /// Enable direct scanout.
    Enable,
    /// Disable direct scanout.
    Disable,
}

#[derive(Args, Debug)]
pub struct OutputArgs {
    /// The output to modify, e.g. DP-1.
    pub output: String,
    #[clap(subcommand)]
    pub command: OutputCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum OutputCommand {
    /// Modify the transform of the output.
    Transform(TransformArgs),
    /// Modify the scale of the output.
    Scale(ScaleArgs),
    /// Modify the mode of the output.
    Mode(ModeArgs),
    /// Modify the position of the output.
    Position(PositionArgs),
    /// Enable the output.
    Enable,
    /// Disable the output.
    Disable,
    /// Override the display's non-desktop setting.
    NonDesktop(NonDesktopArgs),
    /// Change VRR settings.
    Vrr(VrrArgs),
    /// Change tearing settings.
    Tearing(TearingArgs),
    /// Change format settings.
    Format(FormatSettings),
    /// Change color settings.
    Colors(ColorsSettings),
    /// Change the output brightness.
    Brightness(BrightnessArgs),
}

#[derive(ValueEnum, Debug, Clone)]
pub enum NonDesktopType {
    Default,
    False,
    True,
}

#[derive(Args, Debug, Clone)]
pub struct NonDesktopArgs {
    /// Whether this output is a non-desktop output.
    pub setting: NonDesktopType,
}

#[derive(Args, Debug, Clone)]
pub struct VrrArgs {
    #[clap(subcommand)]
    pub command: VrrCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum VrrCommand {
    /// Sets the mode that determines when VRR is enabled.
    SetMode(SetVrrModeArgs),
    /// Sets the maximum refresh rate of the cursor.
    SetCursorHz(CursorHzArgs),
}

#[derive(Args, Debug, Clone)]
pub struct SetVrrModeArgs {
    #[clap(value_enum)]
    pub mode: VrrModeArg,
}

#[derive(ValueEnum, Debug, Copy, Clone, Hash, PartialEq)]
pub enum VrrModeArg {
    /// VRR is never enabled.
    Never,
    /// VRR is always enabled.
    Always,
    /// VRR is enabled when one or more applications are displayed fullscreen.
    Variant1,
    /// VRR is enabled when a single application is displayed fullscreen.
    Variant2,
    /// VRR is enabled when a single game or video is displayed fullscreen.
    Variant3,
}

#[derive(Args, Debug, Clone)]
pub struct CursorHzArgs {
    /// The rate at which the cursor will be updated on screen.
    pub rate: String,
}

#[derive(Args, Debug, Clone)]
pub struct FormatSettings {
    #[clap(subcommand)]
    pub command: FormatCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum FormatCommand {
    /// Sets the format of the framebuffer.
    Set {
        #[clap(value_enum)]
        format: &'static Format,
    },
}

#[derive(Args, Debug, Clone)]
pub struct TearingArgs {
    #[clap(subcommand)]
    pub command: TearingCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum TearingCommand {
    /// Sets the mode that determines when tearing is enabled.
    SetMode(SetTearingModeArgs),
}

#[derive(Args, Debug, Clone)]
pub struct SetTearingModeArgs {
    #[clap(value_enum)]
    pub mode: TearingModeArg,
}

#[derive(ValueEnum, Debug, Copy, Clone, Hash, PartialEq)]
pub enum TearingModeArg {
    /// Tearing is never enabled.
    Never,
    /// Tearing is always enabled.
    Always,
    /// Tearing is enabled when one or more applications are displayed fullscreen.
    Variant1,
    /// Tearing is enabled when a single application is displayed fullscreen.
    Variant2,
    /// Tearing is enabled when a single application is displayed fullscreen and the
    /// application has requested tearing.
    ///
    /// This is the default.
    Variant3,
}

#[derive(Args, Debug, Clone)]
pub struct PositionArgs {
    /// The top-left x coordinate.
    pub x: i32,
    /// The top-left y coordinate.
    pub y: i32,
}

#[derive(Args, Debug, Clone)]
pub struct ModeArgs {
    /// The width.
    pub width: i32,
    /// The height.
    pub height: i32,
    /// The refresh rate.
    pub refresh_rate: f64,
}

#[derive(Args, Debug, Clone)]
pub struct ScaleArgs {
    /// The new scale.
    pub scale: f64,
}

#[derive(Args, Debug, Clone)]
pub struct TransformArgs {
    #[clap(subcommand)]
    pub command: TransformCmd,
}

#[derive(Subcommand, Debug, Clone)]
pub enum TransformCmd {
    /// Apply no transformation.
    None,
    /// Rotate the content 90 degrees counter-clockwise.
    #[clap(name = "rotate-90")]
    Rotate90,
    /// Rotate the content 180 degrees counter-clockwise.
    #[clap(name = "rotate-180")]
    Rotate180,
    /// Rotate the content 270 degrees counter-clockwise.
    #[clap(name = "rotate-270")]
    Rotate270,
    /// Flip the content around the vertical axis.
    Flip,
    /// Flip the content around the vertical axis, then rotate 90 degrees counter-clockwise.
    #[clap(name = "flip-rotate-90")]
    FlipRotate90,
    /// Flip the content around the vertical axis, then rotate 180 degrees counter-clockwise.
    #[clap(name = "flip-rotate-180")]
    FlipRotate180,
    /// Flip the content around the vertical axis, then rotate 270 degrees counter-clockwise.
    #[clap(name = "flip-rotate-270")]
    FlipRotate270,
}

#[derive(Args, Debug, Clone)]
pub struct ColorsSettings {
    #[clap(subcommand)]
    pub command: ColorsCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ColorsCommand {
    /// Sets the color space and transfer function of the output.
    Set {
        /// The name of the color space.
        #[clap(value_parser = PossibleValuesParser::new(color_space_possible_values()))]
        color_space: String,
        /// The name of the transfer function.
        #[clap(value_parser = PossibleValuesParser::new(transfer_function_possible_values()))]
        transfer_function: String,
    },
}

fn color_space_possible_values() -> Vec<PossibleValue> {
    let mut res = vec![];
    for cs in BackendColorSpace::variants() {
        use BackendColorSpace::*;
        let help = match cs {
            Default => "The default color space (usually sRGB)",
            Bt2020 => "The BT.2020 color space",
        };
        res.push(PossibleValue::new(cs.name()).help(help));
    }
    res
}

fn transfer_function_possible_values() -> Vec<PossibleValue> {
    let mut res = vec![];
    for cs in BackendTransferFunction::variants() {
        use BackendTransferFunction::*;
        let help = match cs {
            Default => "The default transfer function (usually sRGB)",
            Pq => "The PQ transfer function",
        };
        res.push(PossibleValue::new(cs.name()).help(help));
    }
    res
}

#[derive(Args, Debug, Clone)]
pub struct BrightnessArgs {
    /// The brightness of standard white in cd/m^2 or `default` to use the default
    /// brightness.
    ///
    /// The default brightness depends on the transfer function:
    ///
    /// - default: the maximum display brightness
    /// - PQ: 203 cd/m^2.
    ///
    /// When using the default transfer function, you likely want to set this to `default`
    /// and adjust the display hardware brightness setting instead.
    ///
    /// This has no effect unless the vulkan renderer is used.
    #[clap(verbatim_doc_comment, value_parser = parse_brightness)]
    brightness: Brightness,
}

#[derive(Debug, Clone)]
pub enum Brightness {
    Default,
    Lux(f64),
}

#[derive(Debug, Error)]
#[error("Value is neither `default` nor a floating point value")]
struct ParseBrightnessError;

fn parse_brightness(s: &str) -> Result<Brightness, ParseBrightnessError> {
    if s == "default" {
        return Ok(Brightness::Default);
    }
    f64::from_str(s)
        .map(Brightness::Lux)
        .map_err(|_| ParseBrightnessError)
}

pub fn main(global: GlobalArgs, args: RandrArgs) {
    with_tool_client(global.log_level.into(), |tc| async move {
        let idle = Rc::new(Randr { tc: tc.clone() });
        idle.run(args).await;
    });
}

#[derive(Clone, Debug)]
struct Device {
    pub id: u64,
    pub syspath: String,
    pub devnode: String,
    pub vendor: u32,
    pub vendor_name: String,
    pub model: u32,
    pub model_name: String,
    pub gfx_api: String,
    pub render_device: bool,
}

#[derive(Clone, Debug)]
struct Connector {
    pub _id: u64,
    pub drm_device: Option<u64>,
    pub name: String,
    pub enabled: bool,
    pub output: Option<Output>,
}

#[derive(Clone, Debug, Default)]
struct Output {
    pub scale: f64,
    pub width: i32,
    pub height: i32,
    pub x: i32,
    pub y: i32,
    pub transform: Transform,
    pub manufacturer: String,
    pub product: String,
    pub serial_number: String,
    pub width_mm: i32,
    pub height_mm: i32,
    pub current_mode: Option<Mode>,
    pub modes: Vec<Mode>,
    pub non_desktop: bool,
    pub vrr_capable: bool,
    pub vrr_enabled: bool,
    pub vrr_mode: VrrMode,
    pub vrr_cursor_hz: Option<f64>,
    pub tearing_mode: TearingMode,
    pub formats: Vec<String>,
    pub format: Option<String>,
    pub flip_margin_ns: Option<u64>,
    pub supported_color_spaces: Vec<String>,
    pub current_color_space: Option<String>,
    pub supported_transfer_functions: Vec<String>,
    pub current_transfer_function: Option<String>,
    pub brightness_range: Option<(f64, f64)>,
    pub brightness: Option<f64>,
}

#[derive(Copy, Clone, Debug)]
struct Mode {
    pub width: i32,
    pub height: i32,
    pub refresh_rate_millihz: u32,
    pub current: bool,
}

impl Mode {
    fn refresh_rate(&self) -> f64 {
        (self.refresh_rate_millihz as f64) / 1000.0
    }
}

impl Display for Mode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} x {} @ {}",
            self.width,
            self.height,
            self.refresh_rate(),
        )
    }
}

#[derive(Clone, Debug, Default)]
struct Data {
    default_api: String,
    drm_devices: Vec<Device>,
    connectors: Vec<Connector>,
}

struct Randr {
    tc: Rc<ToolClient>,
}

impl Randr {
    async fn run(self: &Rc<Self>, args: RandrArgs) {
        let tc = &self.tc;
        let comp = tc.jay_compositor().await;
        let randr = tc.id();
        tc.send(jay_compositor::GetRandr {
            self_id: comp,
            id: randr,
        });
        match args.command.unwrap_or_default() {
            RandrCmd::Show(args) => self.show(randr, args).await,
            RandrCmd::Card(args) => self.card(randr, args).await,
            RandrCmd::Output(args) => self.output(randr, args).await,
        }
    }

    fn handle_error<F: Fn(&str) + 'static>(&self, randr: JayRandrId, f: F) {
        jay_randr::Error::handle(&self.tc, randr, (), move |_, msg| {
            f(msg.msg);
            std::process::exit(1);
        });
    }

    async fn output(self: &Rc<Self>, randr: JayRandrId, args: OutputArgs) {
        let tc = &self.tc;
        match args.command {
            OutputCommand::Transform(t) => {
                self.handle_error(randr, |msg| {
                    eprintln!("Could not modify the transform: {}", msg);
                });
                let transform = match t.command {
                    TransformCmd::None => Transform::None,
                    TransformCmd::Rotate90 => Transform::Rotate90,
                    TransformCmd::Rotate180 => Transform::Rotate180,
                    TransformCmd::Rotate270 => Transform::Rotate270,
                    TransformCmd::Flip => Transform::Flip,
                    TransformCmd::FlipRotate90 => Transform::FlipRotate90,
                    TransformCmd::FlipRotate180 => Transform::FlipRotate180,
                    TransformCmd::FlipRotate270 => Transform::FlipRotate270,
                };
                tc.send(jay_randr::SetTransform {
                    self_id: randr,
                    output: &args.output,
                    transform: transform.to_wl(),
                });
            }
            OutputCommand::Scale(t) => {
                self.handle_error(randr, |msg| {
                    eprintln!("Could not modify the scale: {}", msg);
                });
                let scale = Scale::from_f64(t.scale);
                tc.send(jay_randr::SetScale {
                    self_id: randr,
                    output: &args.output,
                    scale: scale.to_wl(),
                });
            }
            OutputCommand::Mode(t) => {
                let name = args.output.to_ascii_lowercase();
                let data = self.get(randr).await;
                let Some(connector) = data
                    .connectors
                    .iter()
                    .find(|c| c.name.to_ascii_lowercase() == name)
                else {
                    log::error!("Connector with name `{}` does not exist", args.output);
                    return;
                };
                let Some(output) = &connector.output else {
                    log::error!("Connector {} is not connected", connector.name);
                    return;
                };
                let Some(mode) = output.modes.iter().find(|m| {
                    m.width == t.width && m.height == t.height && m.refresh_rate() == t.refresh_rate
                }) else {
                    log::error!(
                        "Output {} does not support this refresh rate",
                        connector.name
                    );
                    return;
                };
                self.handle_error(randr, |msg| {
                    eprintln!("Could not modify the mode: {}", msg);
                });
                tc.send(jay_randr::SetMode {
                    self_id: randr,
                    output: &args.output,
                    width: mode.width,
                    height: mode.height,
                    refresh_rate_millihz: mode.refresh_rate_millihz,
                });
            }
            OutputCommand::Position(t) => {
                self.handle_error(randr, |msg| {
                    eprintln!("Could not modify the position: {}", msg);
                });
                tc.send(jay_randr::SetPosition {
                    self_id: randr,
                    output: &args.output,
                    x: t.x,
                    y: t.y,
                });
            }
            OutputCommand::Enable | OutputCommand::Disable => {
                let (enable, name) = match args.command {
                    OutputCommand::Enable => (true, "enable"),
                    _ => (false, "disable"),
                };
                self.handle_error(randr, move |msg| {
                    eprintln!("Could not {} the output: {}", name, msg);
                });
                tc.send(jay_randr::SetEnabled {
                    self_id: randr,
                    output: &args.output,
                    enabled: enable as _,
                });
            }
            OutputCommand::NonDesktop(a) => {
                self.handle_error(randr, move |msg| {
                    eprintln!("Could not change the non-desktop setting: {}", msg);
                });
                tc.send(jay_randr::SetNonDesktop {
                    self_id: randr,
                    output: &args.output,
                    non_desktop: a.setting as _,
                });
            }
            OutputCommand::Vrr(a) => {
                self.handle_error(randr, move |msg| {
                    eprintln!("Could not change the VRR setting: {}", msg);
                });
                let parse_rate = |rate: &str| {
                    if rate.eq_ignore_ascii_case("none") {
                        f64::INFINITY
                    } else {
                        match f64::from_str(rate) {
                            Ok(v) => v,
                            Err(e) => {
                                fatal!("Could not parse rate: {}", ErrorFmt(e));
                            }
                        }
                    }
                };
                match a.command {
                    VrrCommand::SetMode(a) => {
                        let mode = match a.mode {
                            VrrModeArg::Never => VrrMode::NEVER,
                            VrrModeArg::Always => VrrMode::ALWAYS,
                            VrrModeArg::Variant1 => VrrMode::VARIANT_1,
                            VrrModeArg::Variant2 => VrrMode::VARIANT_2,
                            VrrModeArg::Variant3 => VrrMode::VARIANT_3,
                        };
                        tc.send(jay_randr::SetVrrMode {
                            self_id: randr,
                            output: &args.output,
                            mode: mode.0,
                        });
                    }
                    VrrCommand::SetCursorHz(r) => {
                        let hz = parse_rate(&r.rate);
                        tc.send(jay_randr::SetVrrCursorHz {
                            self_id: randr,
                            output: &args.output,
                            hz,
                        });
                    }
                }
            }
            OutputCommand::Tearing(a) => {
                self.handle_error(randr, move |msg| {
                    eprintln!("Could not change the tearing setting: {}", msg);
                });
                match a.command {
                    TearingCommand::SetMode(a) => {
                        let mode = match a.mode {
                            TearingModeArg::Never => VrrMode::NEVER,
                            TearingModeArg::Always => VrrMode::ALWAYS,
                            TearingModeArg::Variant1 => VrrMode::VARIANT_1,
                            TearingModeArg::Variant2 => VrrMode::VARIANT_2,
                            TearingModeArg::Variant3 => VrrMode::VARIANT_3,
                        };
                        tc.send(jay_randr::SetTearingMode {
                            self_id: randr,
                            output: &args.output,
                            mode: mode.0,
                        });
                    }
                }
            }
            OutputCommand::Format(a) => {
                self.handle_error(randr, move |msg| {
                    eprintln!("Could not change the framebuffer format: {}", msg);
                });
                match a.command {
                    FormatCommand::Set { format } => {
                        tc.send(jay_randr::SetFbFormat {
                            self_id: randr,
                            output: &args.output,
                            format: format.name,
                        });
                    }
                }
            }
            OutputCommand::Colors(a) => {
                self.handle_error(randr, move |msg| {
                    eprintln!("Could not change the colors: {}", msg);
                });
                match a.command {
                    ColorsCommand::Set {
                        color_space,
                        transfer_function,
                    } => {
                        tc.send(jay_randr::SetColors {
                            self_id: randr,
                            output: &args.output,
                            color_space: &color_space,
                            transfer_function: &transfer_function,
                        });
                    }
                }
            }
            OutputCommand::Brightness(a) => {
                self.handle_error(randr, move |msg| {
                    eprintln!("Could not change the brightness: {}", msg);
                });
                match a.brightness {
                    Brightness::Default => {
                        tc.send(jay_randr::UnsetBrightness {
                            self_id: randr,
                            output: &args.output,
                        });
                    }
                    Brightness::Lux(lux) => {
                        tc.send(jay_randr::SetBrightness {
                            self_id: randr,
                            output: &args.output,
                            lux,
                        });
                    }
                }
            }
        }
        tc.round_trip().await;
    }

    async fn card(self: &Rc<Self>, randr: JayRandrId, args: CardArgs) {
        let tc = &self.tc;
        match args.command {
            CardCommand::Primary => {
                self.handle_error(randr, |msg| {
                    eprintln!("Could not set the primary device: {}", msg);
                });
                tc.send(jay_randr::MakeRenderDevice {
                    self_id: randr,
                    dev: &args.card,
                });
            }
            CardCommand::Api(api) => {
                self.handle_error(randr, |msg| {
                    eprintln!("Could not set the API: {}", msg);
                });
                let api = match &api.cmd {
                    ApiCmd::OpenGl => "opengl",
                    ApiCmd::Vulkan => "vulkan",
                };
                tc.send(jay_randr::SetApi {
                    self_id: randr,
                    dev: &args.card,
                    api,
                });
            }
            CardCommand::DirectScanout(ds) => {
                self.handle_error(randr, |msg| {
                    eprintln!("Could not modify direct-scanout behavior: {}", msg);
                });
                tc.send(jay_randr::SetDirectScanout {
                    self_id: randr,
                    dev: &args.card,
                    enabled: match ds.cmd {
                        DirectScanoutCmd::Enable => 1,
                        DirectScanoutCmd::Disable => 0,
                    },
                });
            }
            CardCommand::Timing(ts) => match ts.cmd {
                TimingCmd::SetFlipMargin(sfm) => {
                    self.handle_error(randr, |msg| {
                        eprintln!("Could not modify the flip margin: {}", msg);
                    });
                    tc.send(jay_randr::SetFlipMargin {
                        self_id: randr,
                        dev: &args.card,
                        margin_ns: (sfm.margin_ms * 1_000_000.0) as u64,
                    });
                }
            },
        }
        tc.round_trip().await;
    }

    async fn show(self: &Rc<Self>, randr: JayRandrId, args: ShowArgs) {
        let mut data = self.get(randr).await;
        data.drm_devices.sort_by(|l, r| l.devnode.cmp(&r.devnode));
        if data.drm_devices.is_not_empty() {
            println!("drm devices:");
        }
        for dev in &data.drm_devices {
            self.print_drm_device(dev);
            println!("    connectors:");
            let mut connectors: Vec<_> = data
                .connectors
                .iter()
                .filter(|c| c.drm_device == Some(dev.id))
                .collect();
            connectors.sort_by_key(|c| &c.name);
            for c in connectors {
                self.print_connector(c, args.modes, args.formats);
            }
        }
        {
            let mut connectors: Vec<_> = data
                .connectors
                .iter()
                .filter(|c| c.drm_device.is_none())
                .collect();
            if connectors.is_not_empty() {
                connectors.sort_by_key(|c| &c.name);
                println!("unbound connectors:");
                for c in connectors {
                    self.print_connector(c, args.modes, args.formats);
                }
            }
        }
    }

    fn print_drm_device(&self, dev: &Device) {
        println!("  {}:", dev.devnode);
        println!("    model: {} {}", dev.vendor_name, dev.model_name);
        println!("    pci-id: {:x}:{:x}", dev.vendor, dev.model);
        println!("    syspath: {}", dev.syspath);
        println!("    api: {}", dev.gfx_api);
        if dev.render_device {
            println!("    primary device");
        }
    }

    fn print_connector(&self, connector: &Connector, modes: bool, formats: bool) {
        println!("      {}:", connector.name);
        let Some(o) = &connector.output else {
            if !connector.enabled {
                println!("        disabled");
            } else {
                println!("        disconnected");
            }
            return;
        };
        println!("        product: {}", o.product);
        println!("        manufacturer: {}", o.manufacturer);
        println!("        serial number: {}", o.serial_number);
        println!(
            "        physical size: {}mm x {}mm",
            o.width_mm, o.height_mm
        );
        if o.non_desktop {
            println!("        non-desktop");
            return;
        }
        println!("        VRR capable: {}", o.vrr_capable);
        if o.vrr_capable {
            println!("        VRR enabled: {}", o.vrr_enabled);
            let mode_str;
            let mode = match o.vrr_mode {
                VrrMode::NEVER => "never",
                VrrMode::ALWAYS => "always",
                VrrMode::VARIANT_1 => "variant1",
                VrrMode::VARIANT_2 => "variant2",
                VrrMode::VARIANT_3 => "variant3",
                _ => {
                    mode_str = format!("unknown ({})", o.vrr_mode.0);
                    &mode_str
                }
            };
            println!("        VRR mode: {}", mode);
            if let Some(hz) = o.vrr_cursor_hz {
                println!("        VRR cursor hz: {}", hz);
            }
        }
        {
            let mode_str;
            let mode = match o.tearing_mode {
                TearingMode::NEVER => "never",
                TearingMode::ALWAYS => "always",
                TearingMode::VARIANT_1 => "variant1",
                TearingMode::VARIANT_2 => "variant2",
                TearingMode::VARIANT_3 => "variant3",
                _ => {
                    mode_str = format!("unknown ({})", o.vrr_mode.0);
                    &mode_str
                }
            };
            println!("        Tearing mode: {}", mode);
        }
        println!("        position: {} x {}", o.x, o.y);
        println!("        logical size: {} x {}", o.width, o.height);
        if let Some(mode) = &o.current_mode {
            print!("        mode: ");
            self.print_mode(mode, false);
        }
        if let Some(format) = &o.format {
            if format != XRGB8888.name {
                println!("        format: {format}");
            }
        }
        if o.scale != 1.0 {
            println!("        scale: {}", o.scale);
        }
        if o.transform != Transform::None {
            let name = match o.transform {
                Transform::None => "none",
                Transform::Rotate90 => "rotate-90",
                Transform::Rotate180 => "rotate-180",
                Transform::Rotate270 => "rotate-270",
                Transform::Flip => "flip",
                Transform::FlipRotate90 => "flip-rotate-90",
                Transform::FlipRotate180 => "flip-rotate-180",
                Transform::FlipRotate270 => "flip-rotate-270",
            };
            println!("        transform: {}", name);
        }
        if let Some(flip_margin_ns) = o.flip_margin_ns {
            if flip_margin_ns != 1_500_000 {
                println!(
                    "        flip margin: {:?}",
                    Duration::from_nanos(flip_margin_ns)
                );
            }
        }
        if o.supported_color_spaces.is_not_empty() {
            println!("        color spaces:");
            let handle_cs = |cs: &str| {
                let current = match Some(cs) == o.current_color_space.as_deref() {
                    false => "",
                    true => " (current)",
                };
                println!("          {cs}{current}");
            };
            handle_cs("default");
            o.supported_color_spaces.iter().for_each(|cs| handle_cs(cs));
        }
        if o.supported_transfer_functions.is_not_empty() {
            println!("        transfer functions:");
            let handle_tf = |tf: &str| {
                let current = match Some(tf) == o.current_transfer_function.as_deref() {
                    false => "",
                    true => " (current)",
                };
                println!("          {tf}{current}");
            };
            handle_tf("default");
            o.supported_transfer_functions
                .iter()
                .for_each(|tf| handle_tf(tf));
        }
        if let Some((min, max)) = o.brightness_range {
            println!("        min brightness: {:>10.4} cd/m^2", min);
            println!("        max brightness: {:>10.4} cd/m^2", max);
        } else {
            println!("        max brightness: {:>10.4} cd/m^2 (implied)", 80.0);
        }
        if let Some(lux) = o.brightness {
            println!("        brightness:     {:>10.4} cd/m^2", lux);
        }
        if o.modes.is_not_empty() && modes {
            println!("        modes:");
            for mode in &o.modes {
                print!("          ");
                self.print_mode(mode, true);
            }
        }
        if o.formats.is_not_empty() && formats {
            println!("        formats:");
            for format in &o.formats {
                println!("          {format}");
            }
        }
    }

    fn print_mode(&self, m: &Mode, print_current: bool) {
        print!("{}", m);
        if print_current && m.current {
            print!(" (current)");
        }
        println!();
    }

    async fn get(self: &Rc<Self>, randr: JayRandrId) -> Data {
        let tc = &self.tc;
        tc.send(jay_randr::Get { self_id: randr });
        let data = Rc::new(RefCell::new(Data::default()));
        jay_randr::Global::handle(tc, randr, data.clone(), |data, msg| {
            let mut data = data.borrow_mut();
            data.default_api = msg.default_gfx_api.to_string();
        });
        jay_randr::DrmDevice::handle(tc, randr, data.clone(), |data, msg| {
            data.borrow_mut().drm_devices.push(Device {
                id: msg.id,
                syspath: msg.syspath.to_string(),
                devnode: msg.devnode.to_string(),
                vendor: msg.vendor,
                vendor_name: msg.vendor_name.to_string(),
                model: msg.model,
                model_name: msg.model_name.to_string(),
                gfx_api: msg.gfx_api.to_string(),
                render_device: msg.render_device != 0,
            });
        });
        jay_randr::Connector::handle(tc, randr, data.clone(), |data, msg| {
            let mut data = data.borrow_mut();
            data.connectors.push(Connector {
                _id: msg.id,
                drm_device: (msg.drm_device != 0).then_some(msg.drm_device),
                name: msg.name.to_string(),
                enabled: msg.enabled != 0,
                output: None,
            });
        });
        jay_randr::Output::handle(tc, randr, data.clone(), |data, msg| {
            let mut data = data.borrow_mut();
            let c = data.connectors.last_mut().unwrap();
            c.output = Some(Output {
                scale: Scale::from_wl(msg.scale).to_f64(),
                width: msg.width,
                height: msg.height,
                x: msg.x,
                y: msg.y,
                transform: Transform::from_wl(msg.transform).unwrap(),
                manufacturer: msg.manufacturer.to_string(),
                product: msg.product.to_string(),
                serial_number: msg.serial_number.to_string(),
                width_mm: msg.width_mm,
                height_mm: msg.height_mm,
                ..Default::default()
            });
        });
        jay_randr::NonDesktopOutput::handle(tc, randr, data.clone(), |data, msg| {
            let mut data = data.borrow_mut();
            let c = data.connectors.last_mut().unwrap();
            c.output = Some(Output {
                scale: 1.0,
                manufacturer: msg.manufacturer.to_string(),
                product: msg.product.to_string(),
                serial_number: msg.serial_number.to_string(),
                width_mm: msg.width_mm,
                height_mm: msg.height_mm,
                non_desktop: true,
                ..Default::default()
            });
        });
        jay_randr::VrrState::handle(tc, randr, data.clone(), |data, msg| {
            let mut data = data.borrow_mut();
            let c = data.connectors.last_mut().unwrap();
            let output = c.output.as_mut().unwrap();
            output.vrr_capable = msg.capable != 0;
            output.vrr_enabled = msg.enabled != 0;
            output.vrr_mode = VrrMode(msg.mode);
        });
        jay_randr::VrrCursorHz::handle(tc, randr, data.clone(), move |data, msg| {
            let mut data = data.borrow_mut();
            let c = data.connectors.last_mut().unwrap();
            let output = c.output.as_mut().unwrap();
            output.vrr_cursor_hz = Some(msg.hz);
        });
        jay_randr::TearingState::handle(tc, randr, data.clone(), |data, msg| {
            let mut data = data.borrow_mut();
            let c = data.connectors.last_mut().unwrap();
            let output = c.output.as_mut().unwrap();
            output.tearing_mode = TearingMode(msg.mode);
        });
        jay_randr::FbFormat::handle(tc, randr, data.clone(), |data, msg| {
            let mut data = data.borrow_mut();
            let c = data.connectors.last_mut().unwrap();
            let output = c.output.as_mut().unwrap();
            output.formats.push(msg.name.to_string());
            if msg.current != 0 {
                output.format = Some(msg.name.to_string());
            }
        });
        jay_randr::FlipMargin::handle(tc, randr, data.clone(), |data, msg| {
            let mut data = data.borrow_mut();
            let c = data.connectors.last_mut().unwrap();
            let output = c.output.as_mut().unwrap();
            output.flip_margin_ns = Some(msg.margin_ns);
        });
        jay_randr::Mode::handle(tc, randr, data.clone(), |data, msg| {
            let mut data = data.borrow_mut();
            let c = data.connectors.last_mut().unwrap();
            let o = c.output.as_mut().unwrap();
            let mode = Mode {
                width: msg.width,
                height: msg.height,
                refresh_rate_millihz: msg.refresh_rate_millihz,
                current: msg.current != 0,
            };
            if mode.current {
                o.current_mode = Some(mode);
            }
            o.modes.push(mode);
        });
        jay_randr::SupportedColorSpace::handle(tc, randr, data.clone(), |data, msg| {
            let mut data = data.borrow_mut();
            let c = data.connectors.last_mut().unwrap();
            let output = c.output.as_mut().unwrap();
            output
                .supported_color_spaces
                .push(msg.color_space.to_string());
        });
        jay_randr::CurrentColorSpace::handle(tc, randr, data.clone(), |data, msg| {
            let mut data = data.borrow_mut();
            let c = data.connectors.last_mut().unwrap();
            let output = c.output.as_mut().unwrap();
            output.current_color_space = Some(msg.color_space.to_string());
        });
        jay_randr::SupportedTransferFunction::handle(tc, randr, data.clone(), |data, msg| {
            let mut data = data.borrow_mut();
            let c = data.connectors.last_mut().unwrap();
            let output = c.output.as_mut().unwrap();
            output
                .supported_transfer_functions
                .push(msg.transfer_function.to_string());
        });
        jay_randr::CurrentTransferFunction::handle(tc, randr, data.clone(), |data, msg| {
            let mut data = data.borrow_mut();
            let c = data.connectors.last_mut().unwrap();
            let output = c.output.as_mut().unwrap();
            output.current_transfer_function = Some(msg.transfer_function.to_string());
        });
        jay_randr::BrightnessRange::handle(tc, randr, data.clone(), |data, msg| {
            let mut data = data.borrow_mut();
            let c = data.connectors.last_mut().unwrap();
            let output = c.output.as_mut().unwrap();
            output.brightness_range = Some((msg.min, msg.max));
        });
        jay_randr::Brightness::handle(tc, randr, data.clone(), |data, msg| {
            let mut data = data.borrow_mut();
            let c = data.connectors.last_mut().unwrap();
            let output = c.output.as_mut().unwrap();
            output.brightness = Some(msg.lux);
        });
        tc.round_trip().await;
        let x = data.borrow_mut().clone();
        x
    }
}
