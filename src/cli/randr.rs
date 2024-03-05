use {
    crate::{
        cli::GlobalArgs,
        scale::Scale,
        tools::tool_client::{with_tool_client, Handle, ToolClient},
        utils::transform_ext::TransformExt,
        wire::{jay_compositor, jay_randr, JayRandrId},
    },
    clap::{Args, Subcommand},
    isnt::std_1::vec::IsntVecExt,
    jay_config::video::Transform,
    std::{
        cell::RefCell,
        fmt::{Display, Formatter},
        rc::Rc,
    },
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

#[derive(Clone, Debug)]
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
                    scale: scale.0,
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
                self.print_connector(c, args.modes);
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
                    self.print_connector(c, args.modes);
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

    fn print_connector(&self, connector: &Connector, modes: bool) {
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
        println!("        position: {} x {}", o.x, o.y);
        println!("        logical size: {} x {}", o.width, o.height);
        println!(
            "        physical size: {}mm x {}mm",
            o.width_mm, o.height_mm
        );
        if let Some(mode) = &o.current_mode {
            print!("        mode: ");
            self.print_mode(mode, false);
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
        if o.modes.is_not_empty() && modes {
            println!("        modes:");
            for mode in &o.modes {
                print!("          ");
                self.print_mode(mode, true);
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
                scale: Scale(msg.scale).to_f64(),
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
                modes: Default::default(),
                current_mode: None,
            });
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
        tc.round_trip().await;
        let x = data.borrow_mut().clone();
        x
    }
}
