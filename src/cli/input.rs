use {
    crate::{
        backend::{InputDeviceAccelProfile, InputDeviceCapability, InputDeviceClickMethod},
        cli::GlobalArgs,
        clientmem::ClientMem,
        libinput::consts::{
            ConfigClickMethod, LIBINPUT_CONFIG_ACCEL_PROFILE_ADAPTIVE,
            LIBINPUT_CONFIG_ACCEL_PROFILE_FLAT, LIBINPUT_CONFIG_CLICK_METHOD_BUTTON_AREAS,
            LIBINPUT_CONFIG_CLICK_METHOD_CLICKFINGER, LIBINPUT_CONFIG_CLICK_METHOD_NONE,
        },
        tools::tool_client::{Handle, ToolClient, with_tool_client},
        utils::{errorfmt::ErrorFmt, string_ext::StringExt},
        wire::{JayInputId, jay_compositor, jay_input},
    },
    clap::{Args, Subcommand, ValueEnum, ValueHint},
    isnt::std_1::vec::IsntVecExt,
    std::{
        cell::RefCell,
        io::{Read, Write, stdin, stdout},
        mem,
        ops::DerefMut,
        rc::Rc,
    },
    uapi::{OwnedFd, c},
};

#[derive(Args, Debug)]
pub struct InputArgs {
    #[clap(subcommand)]
    pub command: Option<InputCmd>,
}

#[derive(Subcommand, Debug)]
pub enum InputCmd {
    /// Show the current settings.
    Show(ShowArgs),
    /// Modify the settings of a seat.
    Seat(SeatArgs),
    /// Modify the settings of a device.
    Device(DeviceArgs),
}

impl Default for InputCmd {
    fn default() -> Self {
        Self::Show(Default::default())
    }
}

#[derive(Args, Debug, Default)]
pub struct ShowArgs {
    /// Print more information about devices.
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Args, Debug)]
pub struct SeatArgs {
    /// The seat to modify, e.g. default.
    pub seat: String,
    #[clap(subcommand)]
    pub command: Option<SeatCommand>,
}

#[derive(Args, Debug)]
pub struct DeviceArgs {
    /// The ID of the device to modify.
    pub device: u32,
    #[clap(subcommand)]
    pub command: Option<DeviceCommand>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum SeatCommand {
    /// Show information about this seat.
    Show(SeatShowArgs),
    /// Set the repeat rate of the keyboard.
    SetRepeatRate(SetRepeatRateArgs),
    /// Set the keymap.
    SetKeymap(SetKeymapArgs),
    /// Retrieve the keymap.
    Keymap,
    /// Configure whether this seat uses the hardware cursor.
    UseHardwareCursor(UseHardwareCursorArgs),
    /// Set the size of the cursor.
    SetCursorSize(SetCursorSizeArgs),
}

impl Default for SeatCommand {
    fn default() -> Self {
        Self::Show(SeatShowArgs::default())
    }
}

#[derive(Args, Debug, Default, Clone)]
pub struct SeatShowArgs {
    /// Print more information about devices.
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Subcommand, Debug, Clone, Default)]
pub enum DeviceCommand {
    /// Show information about this device.
    #[default]
    Show,
    /// Set the acceleration profile.
    SetAccelProfile(SetAccelProfileArgs),
    /// Set the acceleration speed.
    SetAccelSpeed(SetAccelSpeedArgs),
    /// Set whether tap is enabled.
    SetTapEnabled(SetTapEnabledArgs),
    /// Set whether tap-drag is enabled.
    SetTapDragEnabled(SetTapDragEnabledArgs),
    /// Set whether tap-drag-lock is enabled.
    SetTapDragLockEnabled(SetTapDragLockEnabledArgs),
    /// Set whether the device is left-handed.
    SetLeftHanded(SetLeftHandedArgs),
    /// Set whether the device uses natural scrolling.
    SetNaturalScrolling(SetNaturalScrollingArgs),
    /// Set the pixels to scroll per scroll-wheel dedent.
    SetPxPerWheelScroll(SetPxPerWheelScrollArgs),
    /// Set the transformation matrix.
    SetTransformMatrix(SetTransformMatrixArgs),
    /// Set the keymap of this device.
    SetKeymap(SetKeymapArgs),
    /// Retrieve the keymap of this device.
    Keymap,
    /// Attach the device to a seat.
    Attach(AttachArgs),
    /// Detach the device from its seat.
    Detach,
    /// Maps this device to an output.
    MapToOutput(MapToOutputArgs),
    /// Removes the mapping from this device to an output.
    RemoveMapping,
    /// Set the calibration matrix.
    SetCalibrationMatrix(SetCalibrationMatrixArgs),
    /// Set the click method.
    SetClickMethod(SetClickMethodArgs),
    /// Set whether the device uses middle button emulation.
    SetMiddleButtonEmulation(SetMiddleButtonEmulationArgs),
}

#[derive(ValueEnum, Debug, Clone)]
pub enum AccelProfile {
    Flat,
    Adaptive,
}

#[derive(Args, Debug, Clone)]
pub struct SetAccelProfileArgs {
    /// The profile.
    pub profile: AccelProfile,
}

#[derive(Args, Debug, Clone)]
pub struct SetAccelSpeedArgs {
    /// The speed. Must be in the range \[-1, 1].
    pub speed: f64,
}

#[derive(Args, Debug, Clone)]
pub struct SetTapEnabledArgs {
    /// Whether tap is enabled.
    #[arg(action = clap::ArgAction::Set)]
    pub enabled: bool,
}

#[derive(Args, Debug, Clone)]
pub struct SetTapDragEnabledArgs {
    /// Whether tap-drag is enabled.
    #[arg(action = clap::ArgAction::Set)]
    pub enabled: bool,
}

#[derive(Args, Debug, Clone)]
pub struct SetTapDragLockEnabledArgs {
    /// Whether tap-drag-lock is enabled.
    #[arg(action = clap::ArgAction::Set)]
    pub enabled: bool,
}

#[derive(Args, Debug, Clone)]
pub struct SetLeftHandedArgs {
    /// Whether the device is left handed.
    #[arg(action = clap::ArgAction::Set)]
    pub left_handed: bool,
}

#[derive(Args, Debug, Clone)]
pub struct SetNaturalScrollingArgs {
    /// Whether natural scrolling is enabled.
    #[arg(action = clap::ArgAction::Set)]
    pub natural_scrolling: bool,
}

#[derive(Args, Debug, Clone)]
pub struct SetPxPerWheelScrollArgs {
    /// The number of pixels to scroll.
    pub px: f64,
}

#[derive(Args, Debug, Clone)]
pub struct SetTransformMatrixArgs {
    pub m11: f64,
    pub m12: f64,
    pub m21: f64,
    pub m22: f64,
}

#[derive(Args, Debug, Clone)]
pub struct SetCalibrationMatrixArgs {
    pub m00: f32,
    pub m01: f32,
    pub m02: f32,
    pub m10: f32,
    pub m11: f32,
    pub m12: f32,
}

#[derive(ValueEnum, Debug, Clone)]
pub enum ClickMethod {
    None,
    ButtonAreas,
    Clickfinger,
}

#[derive(Args, Debug, Clone)]
pub struct SetClickMethodArgs {
    /// The method.
    pub method: ClickMethod,
}

#[derive(Args, Debug, Clone)]
pub struct SetMiddleButtonEmulationArgs {
    /// Whether middle button emulation is enabled.
    #[arg(action = clap::ArgAction::Set)]
    pub middle_button_emulation: bool,
}

#[derive(Args, Debug, Clone)]
pub struct MapToOutputArgs {
    /// The output to map to.
    pub output: String,
}

#[derive(Args, Debug, Clone)]
pub struct AttachArgs {
    /// The seat to attach to.
    pub seat: String,
}

#[derive(Args, Debug, Clone)]
pub struct SetRepeatRateArgs {
    /// The number of repeats per second.
    pub rate: i32,
    /// The delay before the first repeat in milliseconds.
    pub delay: i32,
}

#[derive(Args, Debug, Clone)]
pub struct SetCursorSizeArgs {
    /// The size of the cursor.
    pub size: u32,
}

#[derive(Args, Debug, Clone)]
pub struct SetKeymapArgs {
    /// The file to read the keymap from. Omit for stdin.
    #[clap(value_hint = ValueHint::FilePath)]
    pub file: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct UseHardwareCursorArgs {
    /// Whether the seat uses the hardware cursor.
    #[arg(action = clap::ArgAction::Set)]
    pub enabled: bool,
}

pub fn main(global: GlobalArgs, args: InputArgs) {
    with_tool_client(global.log_level.into(), |tc| async move {
        let idle = Rc::new(Input { tc: tc.clone() });
        idle.run(args).await;
    });
}

#[derive(Clone, Debug)]
struct Seat {
    pub name: String,
    pub repeat_rate: i32,
    pub repeat_delay: i32,
    pub hardware_cursor: bool,
}

#[derive(Clone, Debug)]
struct InputDevice {
    pub id: u32,
    pub name: String,
    pub seat: Option<String>,
    pub syspath: Option<String>,
    pub devnode: Option<String>,
    pub capabilities: Vec<InputDeviceCapability>,
    pub accel_profile: Option<InputDeviceAccelProfile>,
    pub accel_speed: Option<f64>,
    pub tap_enabled: Option<bool>,
    pub tap_drag_enabled: Option<bool>,
    pub tap_drag_lock_enabled: Option<bool>,
    pub left_handed: Option<bool>,
    pub natural_scrolling_enabled: Option<bool>,
    pub px_per_wheel_scroll: Option<f64>,
    pub transform_matrix: Option<[[f64; 2]; 2]>,
    pub output: Option<String>,
    pub calibration_matrix: Option<[[f32; 3]; 2]>,
    pub click_method: Option<InputDeviceClickMethod>,
    pub middle_button_emulation_enabled: Option<bool>,
}

#[derive(Clone, Debug, Default)]
struct Data {
    seats: Vec<Seat>,
    input_device: Vec<InputDevice>,
}

struct Input {
    tc: Rc<ToolClient>,
}

impl Input {
    async fn run(self: &Rc<Self>, args: InputArgs) {
        let tc = &self.tc;
        let comp = tc.jay_compositor().await;
        let input = tc.id();
        tc.send(jay_compositor::GetInput {
            self_id: comp,
            id: input,
        });
        match args.command.unwrap_or_default() {
            InputCmd::Show(args) => self.show(input, args).await,
            InputCmd::Seat(args) => self.seat(input, args).await,
            InputCmd::Device(args) => self.device(input, args).await,
        }
    }

    fn handle_error<F: Fn(&str) + 'static>(&self, input: JayInputId, f: F) {
        jay_input::Error::handle(&self.tc, input, (), move |_, msg| {
            f(msg.msg);
            std::process::exit(1);
        });
    }

    fn prepare_keymap(&self, a: &SetKeymapArgs) -> (Rc<OwnedFd>, usize) {
        let map = match &a.file {
            None => {
                let mut map = vec![];
                if let Err(e) = stdin().read_to_end(&mut map) {
                    eprintln!("Could not read from stdin: {}", ErrorFmt(e));
                    std::process::exit(1);
                }
                map
            }
            Some(f) => match std::fs::read(f) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("Could not read {}: {}", f, ErrorFmt(e));
                    std::process::exit(1);
                }
            },
        };
        let mut memfd =
            uapi::memfd_create("keymap", c::MFD_CLOEXEC | c::MFD_ALLOW_SEALING).unwrap();
        memfd.write_all(&map).unwrap();
        uapi::lseek(memfd.raw(), 0, c::SEEK_SET).unwrap();
        uapi::fcntl_add_seals(
            memfd.raw(),
            c::F_SEAL_SEAL | c::F_SEAL_GROW | c::F_SEAL_SHRINK | c::F_SEAL_WRITE,
        )
        .unwrap();
        (Rc::new(memfd), map.len())
    }

    async fn handle_keymap(&self, input: JayInputId) -> Vec<u8> {
        let data = Rc::new(RefCell::new(Vec::new()));
        jay_input::Keymap::handle(&self.tc, input, data.clone(), |d, map| {
            let mem = Rc::new(
                ClientMem::new_private(&map.keymap, map.keymap_len as _, true, None, None).unwrap(),
            )
            .offset(0);
            mem.read(d.borrow_mut().deref_mut()).unwrap();
        });
        self.tc.round_trip().await;
        data.take()
    }

    async fn seat(self: &Rc<Self>, input: JayInputId, args: SeatArgs) {
        let tc = &self.tc;
        match args.command.unwrap_or_default() {
            SeatCommand::Show(a) => {
                self.handle_error(input, |e| {
                    eprintln!("Could not retrieve seat data: {}", e);
                });
                tc.send(jay_input::GetSeat {
                    self_id: input,
                    name: &args.seat,
                });
                let data = self.get(input).await;
                self.print_data(data, a.verbose);
            }
            SeatCommand::SetRepeatRate(a) => {
                self.handle_error(input, |e| {
                    eprintln!("Could not set repeat rate: {}", e);
                });
                tc.send(jay_input::SetRepeatRate {
                    self_id: input,
                    seat: &args.seat,
                    repeat_rate: a.rate,
                    repeat_delay: a.delay,
                });
            }
            SeatCommand::SetKeymap(a) => {
                let (memfd, len) = self.prepare_keymap(&a);
                self.handle_error(input, |e| {
                    eprintln!("Could not set keymap: {}", e);
                });
                tc.send(jay_input::SetKeymap {
                    self_id: input,
                    seat: &args.seat,
                    keymap: memfd,
                    keymap_len: len as _,
                });
            }
            SeatCommand::UseHardwareCursor(a) => {
                self.handle_error(input, |e| {
                    eprintln!("Could not set hardware cursor: {}", e);
                });
                tc.send(jay_input::UseHardwareCursor {
                    self_id: input,
                    seat: &args.seat,
                    use_hardware_cursor: a.enabled as _,
                });
            }
            SeatCommand::Keymap => {
                self.handle_error(input, |e| {
                    eprintln!("Could not retrieve the keymap: {}", e);
                });
                tc.send(jay_input::GetKeymap {
                    self_id: input,
                    seat: &args.seat,
                });
                let map = self.handle_keymap(input).await;
                stdout().write_all(&map).unwrap();
            }
            SeatCommand::SetCursorSize(a) => {
                self.handle_error(input, |e| {
                    eprintln!("Could not set cursor size: {}", e);
                });
                tc.send(jay_input::SetCursorSize {
                    self_id: input,
                    seat: &args.seat,
                    size: a.size,
                });
            }
        }
        tc.round_trip().await;
    }

    async fn device(self: &Rc<Self>, input: JayInputId, args: DeviceArgs) {
        let tc = &self.tc;
        match args.command.unwrap_or_default() {
            DeviceCommand::Show => {
                self.handle_error(input, |e| {
                    eprintln!("Could not retrieve device data: {}", e);
                });
                tc.send(jay_input::GetDevice {
                    self_id: input,
                    id: args.device,
                });
                let data = self.get(input).await;
                for device in &data.input_device {
                    self.print_device("", true, device);
                }
            }
            DeviceCommand::SetAccelProfile(a) => {
                let profile = match a.profile {
                    AccelProfile::Flat => LIBINPUT_CONFIG_ACCEL_PROFILE_FLAT.0,
                    AccelProfile::Adaptive => LIBINPUT_CONFIG_ACCEL_PROFILE_ADAPTIVE.0,
                };
                self.handle_error(input, |e| {
                    eprintln!("Could not set the acceleration profile: {}", e);
                });
                tc.send(jay_input::SetAccelProfile {
                    self_id: input,
                    id: args.device,
                    profile,
                });
            }
            DeviceCommand::SetAccelSpeed(a) => {
                self.handle_error(input, |e| {
                    eprintln!("Could not set the acceleration speed: {}", e);
                });
                tc.send(jay_input::SetAccelSpeed {
                    self_id: input,
                    id: args.device,
                    speed: a.speed,
                });
            }
            DeviceCommand::SetTapEnabled(a) => {
                self.handle_error(input, |e| {
                    eprintln!("Could not modify the tap-enabled setting: {}", e);
                });
                tc.send(jay_input::SetTapEnabled {
                    self_id: input,
                    id: args.device,
                    enabled: a.enabled as _,
                });
            }
            DeviceCommand::SetTapDragEnabled(a) => {
                self.handle_error(input, |e| {
                    eprintln!("Could not modify the tap-drag-enabled setting: {}", e);
                });
                tc.send(jay_input::SetTapDragEnabled {
                    self_id: input,
                    id: args.device,
                    enabled: a.enabled as _,
                });
            }
            DeviceCommand::SetTapDragLockEnabled(a) => {
                self.handle_error(input, |e| {
                    eprintln!("Could not modify the tap-drag-lock-enabled setting: {}", e);
                });
                tc.send(jay_input::SetTapDragLockEnabled {
                    self_id: input,
                    id: args.device,
                    enabled: a.enabled as _,
                });
            }
            DeviceCommand::SetLeftHanded(a) => {
                self.handle_error(input, |e| {
                    eprintln!("Could not modify the left-handed setting: {}", e);
                });
                tc.send(jay_input::SetLeftHanded {
                    self_id: input,
                    id: args.device,
                    enabled: a.left_handed as _,
                });
            }
            DeviceCommand::SetNaturalScrolling(a) => {
                self.handle_error(input, |e| {
                    eprintln!("Could not modify the natural-scrolling setting: {}", e);
                });
                tc.send(jay_input::SetNaturalScrolling {
                    self_id: input,
                    id: args.device,
                    enabled: a.natural_scrolling as _,
                });
            }
            DeviceCommand::SetPxPerWheelScroll(a) => {
                self.handle_error(input, |e| {
                    eprintln!("Could not modify the px-per-wheel-scroll setting: {}", e);
                });
                tc.send(jay_input::SetPxPerWheelScroll {
                    self_id: input,
                    id: args.device,
                    px: a.px,
                });
            }
            DeviceCommand::SetTransformMatrix(a) => {
                self.handle_error(input, |e| {
                    eprintln!("Could not modify the transform matrix: {}", e);
                });
                tc.send(jay_input::SetTransformMatrix {
                    self_id: input,
                    id: args.device,
                    m11: a.m11,
                    m12: a.m12,
                    m21: a.m21,
                    m22: a.m22,
                });
            }
            DeviceCommand::Attach(a) => {
                self.handle_error(input, |e| {
                    eprintln!("Could not attach the device: {}", e);
                });
                tc.send(jay_input::Attach {
                    self_id: input,
                    id: args.device,
                    seat: &a.seat,
                });
            }
            DeviceCommand::Detach => {
                self.handle_error(input, |e| {
                    eprintln!("Could not detach the device: {}", e);
                });
                tc.send(jay_input::Detach {
                    self_id: input,
                    id: args.device,
                });
            }
            DeviceCommand::SetKeymap(a) => {
                let (memfd, len) = self.prepare_keymap(&a);
                self.handle_error(input, |e| {
                    eprintln!("Could not set keymap: {}", e);
                });
                tc.send(jay_input::SetDeviceKeymap {
                    self_id: input,
                    id: args.device,
                    keymap: memfd,
                    keymap_len: len as _,
                });
            }
            DeviceCommand::Keymap => {
                self.handle_error(input, |e| {
                    eprintln!("Could not retrieve the keymap: {}", e);
                });
                tc.send(jay_input::GetDeviceKeymap {
                    self_id: input,
                    id: args.device,
                });
                let map = self.handle_keymap(input).await;
                stdout().write_all(&map).unwrap();
            }
            DeviceCommand::MapToOutput(a) => {
                self.handle_error(input, |e| {
                    eprintln!("Could not map the device to an output: {}", e);
                });
                tc.send(jay_input::MapToOutput {
                    self_id: input,
                    id: args.device,
                    output: Some(&a.output),
                });
            }
            DeviceCommand::RemoveMapping => {
                self.handle_error(input, |e| {
                    eprintln!("Could not remove the output mapping: {}", e);
                });
                tc.send(jay_input::MapToOutput {
                    self_id: input,
                    id: args.device,
                    output: None,
                });
            }
            DeviceCommand::SetCalibrationMatrix(a) => {
                self.handle_error(input, |e| {
                    eprintln!("Could not modify the calibration matrix: {}", e);
                });
                tc.send(jay_input::SetCalibrationMatrix {
                    self_id: input,
                    id: args.device,
                    m00: a.m00,
                    m01: a.m01,
                    m02: a.m02,
                    m10: a.m10,
                    m11: a.m11,
                    m12: a.m12,
                });
            }
            DeviceCommand::SetClickMethod(a) => {
                let method = match a.method {
                    ClickMethod::None => LIBINPUT_CONFIG_CLICK_METHOD_NONE.0,
                    ClickMethod::ButtonAreas => LIBINPUT_CONFIG_CLICK_METHOD_BUTTON_AREAS.0,
                    ClickMethod::Clickfinger => LIBINPUT_CONFIG_CLICK_METHOD_CLICKFINGER.0,
                };
                self.handle_error(input, |e| {
                    eprintln!("Could not set the click method: {}", e);
                });
                tc.send(jay_input::SetClickMethod {
                    self_id: input,
                    id: args.device,
                    method,
                });
            }
            DeviceCommand::SetMiddleButtonEmulation(a) => {
                self.handle_error(input, |e| {
                    eprintln!(
                        "Could not modify the middle-button-emulation setting: {}",
                        e
                    );
                });
                tc.send(jay_input::SetMiddleButtonEmulation {
                    self_id: input,
                    id: args.device,
                    enabled: a.middle_button_emulation as _,
                });
            }
        }
        tc.round_trip().await;
    }

    async fn show(self: &Rc<Self>, input: JayInputId, args: ShowArgs) {
        self.tc.send(jay_input::GetAll { self_id: input });
        let data = self.get(input).await;
        self.print_data(data, args.verbose);
    }

    fn print_data(self: &Rc<Self>, mut data: Data, verbose: bool) {
        data.seats.sort_by(|l, r| l.name.cmp(&r.name));
        data.input_device.sort_by_key(|l| l.id);
        let mut first = true;
        let print_devices = |d: &[&InputDevice]| {
            for device in d {
                if verbose {
                    self.print_device("    ", false, device);
                } else {
                    println!("    {}: {}", device.id, device.name);
                }
            }
        };
        for seat in &data.seats {
            if !mem::take(&mut first) {
                println!();
            }
            self.print_seat(seat);
            let input_devices: Vec<_> = data
                .input_device
                .iter()
                .filter(|c| c.seat.as_ref() == Some(&seat.name))
                .collect();
            if input_devices.is_not_empty() {
                println!("  devices:");
            }
            print_devices(&input_devices);
        }
        {
            let input_devices: Vec<_> = data
                .input_device
                .iter()
                .filter(|c| c.seat.is_none())
                .collect();
            if input_devices.is_not_empty() {
                if !mem::take(&mut first) {
                    println!();
                }
                println!("Detached devices:");
                print_devices(&input_devices);
            }
        }
    }

    fn print_seat(&self, seat: &Seat) {
        println!("Seat {}:", seat.name);
        println!("  repeat rate: {}", seat.repeat_rate);
        println!("  repeat delay: {}", seat.repeat_delay);
        if !seat.hardware_cursor {
            println!("  hardware cursor disabled");
        }
    }

    fn print_device(&self, prefix: &str, print_seat: bool, device: &InputDevice) {
        println!("{prefix}{}:", device.id);
        println!("{prefix}  name: {}", device.name);
        if print_seat {
            let seat = match device.seat.as_deref() {
                Some(s) => s,
                _ => "<detached>",
            };
            println!("{prefix}  seat: {}", seat);
        }
        if let Some(v) = &device.syspath {
            println!("{prefix}  syspath: {}", v);
        }
        if let Some(v) = &device.devnode {
            println!("{prefix}  devnode: {}", v);
        }
        print!("{prefix}  capabilities:");
        let mut first = true;
        for cap in &device.capabilities {
            use InputDeviceCapability::*;
            print!(" ");
            if !mem::take(&mut first) {
                print!("| ");
            }
            let name = match cap {
                Keyboard => "keyboard",
                Pointer => "pointer",
                Touch => "touch",
                TabletTool => "tablet tool",
                TabletPad => "tablet pad",
                Gesture => "gesture",
                Switch => "switch",
            };
            print!("{}", name);
        }
        println!();
        if let Some(v) = &device.accel_profile {
            let name = match v {
                InputDeviceAccelProfile::Flat => "flat",
                InputDeviceAccelProfile::Adaptive => "adaptive",
            };
            println!("{prefix}  accel profile: {}", name);
        }
        if let Some(v) = &device.accel_speed {
            println!("{prefix}  accel speed: {}", v);
        }
        if let Some(v) = &device.tap_enabled {
            println!("{prefix}  tap enabled: {}", v);
        }
        if let Some(v) = &device.tap_drag_enabled {
            println!("{prefix}  tap drag enabled: {}", v);
        }
        if let Some(v) = &device.tap_drag_lock_enabled {
            println!("{prefix}  tap drag lock enabled: {}", v);
        }
        if let Some(v) = &device.left_handed {
            println!("{prefix}  left handed: {}", v);
        }
        if let Some(v) = &device.natural_scrolling_enabled {
            println!("{prefix}  natural scrolling: {}", v);
        }
        if let Some(v) = &device.px_per_wheel_scroll {
            println!("{prefix}  px per wheel scroll: {}", v);
        }
        if let Some(v) = &device.transform_matrix {
            println!("{prefix}  transform matrix: {:?}", v);
        }
        if let Some(v) = &device.output {
            println!("{prefix}  mapped to output: {}", v);
        }
        if let Some(v) = &device.calibration_matrix {
            println!("{prefix}  calibration matrix: {:?}", v);
        }
        if let Some(v) = &device.click_method {
            let name = match v {
                InputDeviceClickMethod::None => "none",
                InputDeviceClickMethod::ButtonAreas => "button-areas",
                InputDeviceClickMethod::Clickfinger => "clickfinger",
            };
            println!("{prefix}  click method: {}", name);
        }
        if let Some(v) = &device.middle_button_emulation_enabled {
            println!("{prefix}  middle button emulation: {}", v);
        }
    }

    async fn get(self: &Rc<Self>, input: JayInputId) -> Data {
        let tc = &self.tc;
        let data = Rc::new(RefCell::new(Data::default()));
        jay_input::Seat::handle(tc, input, data.clone(), |data, msg| {
            data.borrow_mut().seats.push(Seat {
                name: msg.name.to_string(),
                repeat_rate: msg.repeat_rate,
                repeat_delay: msg.repeat_delay,
                hardware_cursor: msg.hardware_cursor != 0,
            });
        });
        jay_input::InputDevice::handle(tc, input, data.clone(), |data, msg| {
            use crate::{backend::InputDeviceCapability::*, libinput::consts::*};
            let mut capabilities = vec![];
            let mut is_pointer = false;
            for cap in msg.capabilities {
                let cap = match DeviceCapability(*cap) {
                    LIBINPUT_DEVICE_CAP_KEYBOARD => Keyboard,
                    LIBINPUT_DEVICE_CAP_POINTER => {
                        is_pointer = true;
                        Pointer
                    }
                    LIBINPUT_DEVICE_CAP_TOUCH => Touch,
                    LIBINPUT_DEVICE_CAP_TABLET_TOOL => TabletTool,
                    LIBINPUT_DEVICE_CAP_TABLET_PAD => TabletPad,
                    LIBINPUT_DEVICE_CAP_GESTURE => Gesture,
                    LIBINPUT_DEVICE_CAP_SWITCH => InputDeviceCapability::Switch,
                    _ => continue,
                };
                capabilities.push(cap);
            }
            let accel_available = msg.accel_available != 0;
            let tap_available = msg.tap_available != 0;
            let left_handed_available = msg.left_handed_available != 0;
            let natural_scrolling_available = msg.natural_scrolling_available != 0;
            let mut accel_profile = match AccelProfile(msg.accel_profile) {
                LIBINPUT_CONFIG_ACCEL_PROFILE_FLAT => Some(InputDeviceAccelProfile::Flat),
                LIBINPUT_CONFIG_ACCEL_PROFILE_ADAPTIVE => Some(InputDeviceAccelProfile::Adaptive),
                _ => None,
            };
            if !accel_available {
                accel_profile = None;
            }
            let mut data = data.borrow_mut();
            data.input_device.push(InputDevice {
                id: msg.id,
                name: msg.name.to_string(),
                seat: msg.seat.to_string_if_not_empty(),
                syspath: msg.syspath.to_string_if_not_empty(),
                devnode: msg.devnode.to_string_if_not_empty(),
                capabilities,
                accel_profile,
                accel_speed: accel_available.then_some(msg.accel_speed),
                tap_enabled: tap_available.then_some(msg.tap_enabled != 0),
                tap_drag_enabled: tap_available.then_some(msg.tap_drag_enabled != 0),
                tap_drag_lock_enabled: tap_available.then_some(msg.tap_drag_lock_enabled != 0),
                left_handed: left_handed_available.then_some(msg.left_handed != 0),
                natural_scrolling_enabled: natural_scrolling_available
                    .then_some(msg.natural_scrolling_enabled != 0),
                px_per_wheel_scroll: is_pointer.then_some(msg.px_per_wheel_scroll),
                transform_matrix: uapi::pod_read(msg.transform_matrix).ok(),
                output: None,
                calibration_matrix: None,
                click_method: None,
                middle_button_emulation_enabled: None,
            });
        });
        jay_input::InputDeviceOutput::handle(tc, input, data.clone(), |data, msg| {
            let mut data = data.borrow_mut();
            if let Some(last) = data.input_device.last_mut() {
                last.output = Some(msg.output.to_string());
            }
        });
        jay_input::CalibrationMatrix::handle(tc, input, data.clone(), |data, msg| {
            let mut data = data.borrow_mut();
            if let Some(last) = data.input_device.last_mut() {
                last.calibration_matrix =
                    Some([[msg.m00, msg.m01, msg.m02], [msg.m10, msg.m11, msg.m12]]);
            }
        });
        jay_input::ClickMethod::handle(tc, input, data.clone(), |data, msg| {
            let click_method = match ConfigClickMethod(msg.click_method) {
                LIBINPUT_CONFIG_CLICK_METHOD_NONE => Some(InputDeviceClickMethod::None),
                LIBINPUT_CONFIG_CLICK_METHOD_BUTTON_AREAS => {
                    Some(InputDeviceClickMethod::ButtonAreas)
                }
                LIBINPUT_CONFIG_CLICK_METHOD_CLICKFINGER => {
                    Some(InputDeviceClickMethod::Clickfinger)
                }
                _ => None,
            };
            let mut data = data.borrow_mut();
            if let Some(last) = data.input_device.last_mut() {
                last.click_method = click_method;
            }
        });
        jay_input::MiddleButtonEmulation::handle(tc, input, data.clone(), |data, msg| {
            let mut data = data.borrow_mut();
            if let Some(last) = data.input_device.last_mut() {
                last.middle_button_emulation_enabled =
                    Some(msg.middle_button_emulation_enabled != 0);
            }
        });
        tc.round_trip().await;
        data.borrow_mut().clone()
    }
}
