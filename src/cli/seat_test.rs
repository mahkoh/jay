use {
    crate::{
        cli::{GlobalArgs, SeatTestArgs},
        fixed::Fixed,
        ifs::wl_seat::wl_pointer::{
            CONTINUOUS, FINGER, HORIZONTAL_SCROLL, PendingScroll, VERTICAL_SCROLL, WHEEL,
        },
        tools::tool_client::{Handle, ToolClient, with_tool_client},
        wire::{
            jay_compositor::{GetSeats, Seat, SeatEvents},
            jay_seat_events::{
                Axis120, AxisFrame, AxisInverted, AxisPx, AxisSource, AxisStop, Button, HoldBegin,
                HoldEnd, Key, Modifiers, PinchBegin, PinchEnd, PinchUpdate, PointerAbs, PointerRel,
                SwipeBegin, SwipeEnd, SwipeUpdate, SwitchEvent, TabletPadButton,
                TabletPadDialDelta, TabletPadDialFrame, TabletPadModeSwitch, TabletPadRingAngle,
                TabletPadRingFrame, TabletPadRingSource, TabletPadRingStop, TabletPadStripFrame,
                TabletPadStripPosition, TabletPadStripSource, TabletPadStripStop, TabletToolButton,
                TabletToolDistance, TabletToolDown, TabletToolFrame, TabletToolMotion,
                TabletToolPressure, TabletToolProximityIn, TabletToolProximityOut,
                TabletToolRotation, TabletToolSlider, TabletToolTilt, TabletToolUp,
                TabletToolWheel, TouchCancel, TouchDown, TouchMotion, TouchUp,
            },
        },
    },
    ahash::AHashMap,
    std::{cell::RefCell, future::pending, ops::Deref, rc::Rc},
};

pub fn main(global: GlobalArgs, args: SeatTestArgs) {
    with_tool_client(global.log_level.into(), |tc| async move {
        let screenshot = Rc::new(SeatTest {
            tc: tc.clone(),
            args,
            names: Default::default(),
        });
        run(screenshot).await;
    });
}

struct SeatTest {
    tc: Rc<ToolClient>,
    args: SeatTestArgs,
    names: RefCell<AHashMap<u32, Rc<String>>>,
}

impl SeatTest {
    fn name(&self, seat: u32) -> Rc<String> {
        match self.names.borrow_mut().get(&seat) {
            Some(n) => n.clone(),
            _ => Rc::new("unknown".to_string()),
        }
    }
}

#[derive(Default, Debug, Copy, Clone)]
pub struct PendingTabletTool {
    proximity_in: bool,
    proximity_out: bool,
    down: bool,
    up: bool,
    pos: Option<(Fixed, Fixed)>,
    pressure: Option<f64>,
    distance: Option<f64>,
    tilt: Option<(f64, f64)>,
    rotation: Option<f64>,
    slider: Option<f64>,
    wheel: Option<(f64, i32)>,
    button: Option<(u32, u32)>,
}

#[derive(Default, Debug, Copy, Clone)]
pub struct PendingTabletPadStrip {
    source: u32,
    pos: Option<f64>,
    stop: bool,
}

#[derive(Default, Debug, Copy, Clone)]
pub struct PendingTabletPadRing {
    source: u32,
    degrees: Option<f64>,
    stop: bool,
}

#[derive(Default, Debug, Copy, Clone)]
pub struct PendingTabletPadDial {
    value120: Option<i32>,
}

async fn run(seat_test: Rc<SeatTest>) {
    let tc = &seat_test.tc;
    let comp = tc.jay_compositor().await;
    tc.send(GetSeats { self_id: comp });
    Seat::handle(tc, comp, seat_test.clone(), |st, seat| {
        st.names
            .borrow_mut()
            .insert(seat.id, Rc::new(seat.name.to_string()));
    });
    tc.round_trip().await;
    let all = seat_test.args.all;
    let mut seat = 0;
    if !all {
        seat = choose_seat(&seat_test);
    }
    let se = tc.id();
    tc.send(SeatEvents {
        self_id: comp,
        id: se,
    });
    let st = seat_test.clone();
    Key::handle(tc, se, (), move |_, ev| {
        if all || ev.seat == seat {
            if all {
                print!("Seat: {}, ", st.name(ev.seat));
            }
            println!(
                "Time: {:.4}, Key: {}, State: {}",
                time(ev.time_usec),
                ev.key,
                ev.state
            );
        }
    });
    let st = seat_test.clone();
    Modifiers::handle(tc, se, (), move |_, ev| {
        if all || ev.seat == seat {
            if all {
                print!("Seat: {}, ", st.name(ev.seat));
            }
            println!("Modifiers: {:08b}, Group: {}", ev.modifiers, ev.group);
        }
    });
    let st = seat_test.clone();
    PointerAbs::handle(tc, se, (), move |_, ev| {
        if all || ev.seat == seat {
            if all {
                print!("Seat: {}, ", st.name(ev.seat));
            }
            println!(
                "Time: {:.4}, Pointer: {}x{}",
                time(ev.time_usec),
                ev.x,
                ev.y
            );
        }
    });
    let st = seat_test.clone();
    PointerRel::handle(tc, se, (), move |_, ev| {
        if all || ev.seat == seat {
            if all {
                print!("Seat: {}, ", st.name(ev.seat));
            }
            println!(
                "Time: {:.4}, Pointer: {:+.4}x{:+.4}, Rel: {:+.4}x{:+.4}, Unaccelerated: {:+.4}x{:+.4}",
                time(ev.time_usec),
                ev.x,
                ev.y,
                ev.dx,
                ev.dy,
                ev.dx_unaccelerated,
                ev.dy_unaccelerated
            );
        }
    });
    let st = seat_test.clone();
    Button::handle(tc, se, (), move |_, ev| {
        if all || ev.seat == seat {
            if all {
                print!("Seat: {:.4}, ", st.name(ev.seat));
            }
            println!(
                "Time: {}, Button: {}, State: {}",
                time(ev.time_usec),
                ev.button,
                ev.state
            );
        }
    });
    let ps = Rc::new(PendingScroll::default());
    AxisSource::handle(tc, se, ps.clone(), move |ps, ev| {
        ps.source.set(Some(ev.source));
    });
    AxisInverted::handle(tc, se, ps.clone(), move |ps, ev| {
        ps.inverted[ev.axis as usize].set(ev.inverted != 0);
    });
    AxisPx::handle(tc, se, ps.clone(), move |ps, ev| {
        ps.px[ev.axis as usize].set(Some(ev.dist));
    });
    AxisStop::handle(tc, se, ps.clone(), move |ps, ev| {
        ps.stop[ev.axis as usize].set(true);
    });
    Axis120::handle(tc, se, ps.clone(), move |ps, ev| {
        ps.v120[ev.axis as usize].set(Some(ev.dist));
    });
    let st = seat_test.clone();
    AxisFrame::handle(tc, se, ps.clone(), move |ps, ev| {
        let source = ps.source.take();
        let px_x = ps.px[HORIZONTAL_SCROLL].take();
        let px_y = ps.px[VERTICAL_SCROLL].take();
        let stop_x = ps.stop[HORIZONTAL_SCROLL].take();
        let stop_y = ps.stop[VERTICAL_SCROLL].take();
        let v120_x = ps.v120[HORIZONTAL_SCROLL].take();
        let v120_y = ps.v120[VERTICAL_SCROLL].take();
        let inverted_x = ps.inverted[HORIZONTAL_SCROLL].get();
        let inverted_y = ps.inverted[VERTICAL_SCROLL].get();
        if all || ev.seat == seat {
            if all {
                print!("Seat: {}, ", st.name(ev.seat));
            }
            let mut need_comma = false;
            macro_rules! comma {
                () => {
                    if std::mem::take(&mut need_comma) {
                        print!(", ");
                    }
                };
            }
            print!("Time: {:.4}, ", time(ev.time_usec));
            if let Some(source) = source {
                let source = match source {
                    WHEEL => "wheel",
                    FINGER => "finger",
                    CONTINUOUS => "continuous",
                    _ => "unknown",
                };
                print!("Source: {}", source);
                need_comma = true;
            }
            for (axis, px, steps, stop, inverted) in [
                ("horizontal", px_x, v120_x, stop_x, inverted_x),
                ("vertical", px_y, v120_y, stop_y, inverted_y),
            ] {
                if px.is_some() || steps.is_some() || stop {
                    comma!();
                    print!("Axis {}: ", axis);
                }
                if let Some(dist) = px {
                    print!("{:+.4}px", dist);
                    need_comma = true;
                }
                if let Some(dist) = steps {
                    comma!();
                    print!("steps: {:+}/120", dist);
                    need_comma = true;
                }
                if stop {
                    comma!();
                    print!("stop");
                    need_comma = true;
                }
                if inverted {
                    comma!();
                    print!("natural scrolling");
                    need_comma = true;
                }
            }
            println!();
        }
    });
    let st = seat_test.clone();
    SwipeBegin::handle(tc, se, (), move |_, ev| {
        if all || ev.seat == seat {
            if all {
                print!("Seat: {}, ", st.name(ev.seat));
            }
            println!(
                "Time: {:.4}, Swipe Begin: {} fingers",
                time(ev.time_usec),
                ev.fingers,
            );
        }
    });
    let st = seat_test.clone();
    SwipeUpdate::handle(tc, se, (), move |_, ev| {
        if all || ev.seat == seat {
            if all {
                print!("Seat: {}, ", st.name(ev.seat));
            }
            println!(
                "Time: {:.4}, Swipe Update: {}x{}, Unaccelerated: {}x{}",
                time(ev.time_usec),
                ev.dx,
                ev.dy,
                ev.dx_unaccelerated,
                ev.dy_unaccelerated,
            );
        }
    });
    let st = seat_test.clone();
    SwipeEnd::handle(tc, se, (), move |_, ev| {
        if all || ev.seat == seat {
            if all {
                print!("Seat: {}, ", st.name(ev.seat));
            }
            print!("Time: {:.4}, Swipe End", time(ev.time_usec),);
            if ev.cancelled != 0 {
                print!(", cancelled");
            }
            println!();
        }
    });
    let st = seat_test.clone();
    PinchBegin::handle(tc, se, (), move |_, ev| {
        if all || ev.seat == seat {
            if all {
                print!("Seat: {}, ", st.name(ev.seat));
            }
            println!(
                "Time: {:.4}, Pinch Begin: {} fingers",
                time(ev.time_usec),
                ev.fingers,
            );
        }
    });
    let st = seat_test.clone();
    PinchUpdate::handle(tc, se, (), move |_, ev| {
        if all || ev.seat == seat {
            if all {
                print!("Seat: {}, ", st.name(ev.seat));
            }
            println!(
                "Time: {:.4}, Pinch Update: {}x{}, Unaccelerated: {}x{}, Scale: {}, Rotation: {}",
                time(ev.time_usec),
                ev.dx,
                ev.dy,
                ev.dx_unaccelerated,
                ev.dy_unaccelerated,
                ev.scale,
                ev.rotation,
            );
        }
    });
    let st = seat_test.clone();
    PinchEnd::handle(tc, se, (), move |_, ev| {
        if all || ev.seat == seat {
            if all {
                print!("Seat: {}, ", st.name(ev.seat));
            }
            print!("Time: {:.4}, Pinch End", time(ev.time_usec));
            if ev.cancelled != 0 {
                print!(", cancelled");
            }
            println!();
        }
    });
    let st = seat_test.clone();
    HoldBegin::handle(tc, se, (), move |_, ev| {
        if all || ev.seat == seat {
            if all {
                print!("Seat: {}, ", st.name(ev.seat));
            }
            println!(
                "Time: {:.4}, Hold Begin: {} fingers",
                time(ev.time_usec),
                ev.fingers,
            );
        }
    });
    let st = seat_test.clone();
    HoldEnd::handle(tc, se, (), move |_, ev| {
        if all || ev.seat == seat {
            if all {
                print!("Seat: {}, ", st.name(ev.seat));
            }
            print!("Time: {:.4}, Hold End", time(ev.time_usec));
            if ev.cancelled != 0 {
                print!(", cancelled");
            }
            println!();
        }
    });
    let st = seat_test.clone();
    SwitchEvent::handle(tc, se, (), move |_, ev| {
        let event = match ev.event {
            0 => "lid opened",
            1 => "lid closed",
            2 => "converted to laptop",
            3 => "converted to tablet",
            _ => "unknown event",
        };
        if all || ev.seat == seat {
            if all {
                print!("Seat: {}, ", st.name(ev.seat));
            }
            println!(
                "Time: {:.4}, Device: {}, {event}",
                time(ev.time_usec),
                ev.input_device
            );
        }
    });
    let tt = Rc::new(RefCell::new(PendingTabletTool::default()));
    TabletToolProximityIn::handle(tc, se, tt.clone(), move |tt, _| {
        tt.borrow_mut().proximity_in = true;
    });
    TabletToolProximityOut::handle(tc, se, tt.clone(), move |tt, _| {
        tt.borrow_mut().proximity_out = true;
    });
    TabletToolDown::handle(tc, se, tt.clone(), move |tt, _| {
        tt.borrow_mut().down = true;
    });
    TabletToolUp::handle(tc, se, tt.clone(), move |tt, _| {
        tt.borrow_mut().up = true;
    });
    TabletToolMotion::handle(tc, se, tt.clone(), move |tt, ev| {
        tt.borrow_mut().pos = Some((ev.x, ev.y));
    });
    TabletToolPressure::handle(tc, se, tt.clone(), move |tt, ev| {
        tt.borrow_mut().pressure = Some(ev.pressure);
    });
    TabletToolDistance::handle(tc, se, tt.clone(), move |tt, ev| {
        tt.borrow_mut().distance = Some(ev.distance);
    });
    TabletToolTilt::handle(tc, se, tt.clone(), move |tt, ev| {
        tt.borrow_mut().tilt = Some((ev.tilt_x, ev.tilt_y));
    });
    TabletToolRotation::handle(tc, se, tt.clone(), move |tt, ev| {
        tt.borrow_mut().rotation = Some(ev.degrees);
    });
    TabletToolSlider::handle(tc, se, tt.clone(), move |tt, ev| {
        tt.borrow_mut().slider = Some(ev.position);
    });
    TabletToolWheel::handle(tc, se, tt.clone(), move |tt, ev| {
        tt.borrow_mut().wheel = Some((ev.degrees, ev.clicks));
    });
    TabletToolButton::handle(tc, se, tt.clone(), move |tt, ev| {
        tt.borrow_mut().button = Some((ev.button, ev.state));
    });
    let st = seat_test.clone();
    TabletToolFrame::handle(tc, se, tt.clone(), move |tt, ev| {
        let tt = tt.take();
        if !all && ev.seat != seat {
            return;
        }
        if all {
            print!("Seat: {}, ", st.name(ev.seat));
        }
        print!(
            "Time: {:.4}, Device: {}, Tool: {}",
            time(ev.time_usec),
            ev.input_device,
            ev.tool,
        );
        if tt.proximity_in {
            print!(", proximity in");
        }
        if tt.proximity_out {
            print!(", proximity out");
        }
        if tt.down {
            print!(", down");
        }
        if tt.up {
            print!(", up");
        }
        if let Some((x, y)) = tt.pos {
            print!(", pos: {x}x{y}");
        }
        if let Some(val) = tt.pressure {
            print!(", pressure: {val}");
        }
        if let Some(val) = tt.distance {
            print!(", distance: {val}");
        }
        if let Some((x, y)) = tt.tilt {
            print!(", tilt: {x}x{y}");
        }
        if let Some(val) = tt.rotation {
            print!(", rotation: {val}");
        }
        if let Some(val) = tt.slider {
            print!(", slider: {val}");
        }
        if let Some((degrees, clicks)) = tt.wheel {
            print!(", wheel degrees: {degrees}, wheel clicks: {clicks}");
        }
        if let Some((button, state)) = tt.button {
            let dir = match state {
                0 => "up",
                _ => "down",
            };
            print!(", button {button} {dir}");
        }
        println!();
    });
    let st = seat_test.clone();
    TabletPadModeSwitch::handle(tc, se, (), move |_, ev| {
        if all || ev.seat == seat {
            if all {
                print!("Seat: {}, ", st.name(ev.seat));
            }
            println!(
                "Time: {:.4}, Device: {}, mode switch: {}",
                time(ev.time_usec),
                ev.input_device,
                ev.mode,
            );
        }
    });
    let st = seat_test.clone();
    TabletPadButton::handle(tc, se, (), move |_, ev| {
        if all || ev.seat == seat {
            if all {
                print!("Seat: {}, ", st.name(ev.seat));
            }
            let dir = match ev.state {
                0 => "up",
                _ => "down",
            };
            println!(
                "Time: {:.4}, Device: {}, Button {} {dir}",
                time(ev.time_usec),
                ev.input_device,
                ev.button,
            );
        }
    });
    let tt = Rc::new(RefCell::new(PendingTabletPadStrip::default()));
    TabletPadStripSource::handle(tc, se, tt.clone(), move |tt, ev| {
        tt.borrow_mut().source = ev.source;
    });
    TabletPadStripPosition::handle(tc, se, tt.clone(), move |tt, ev| {
        tt.borrow_mut().pos = Some(ev.position);
    });
    TabletPadStripStop::handle(tc, se, tt.clone(), move |tt, _| {
        tt.borrow_mut().stop = true;
    });
    let st = seat_test.clone();
    TabletPadStripFrame::handle(tc, se, tt.clone(), move |tt, ev| {
        let tt = tt.take();
        if !all && ev.seat != seat {
            return;
        }
        if all {
            print!("Seat: {}, ", st.name(ev.seat));
        }
        print!(
            "Time: {:.4}, Device: {}, Strip: {}",
            time(ev.time_usec),
            ev.input_device,
            ev.strip,
        );
        let source = match tt.source {
            1 => "finger",
            _ => "unknown",
        };
        print!(", source: {source}");
        if let Some(pos) = tt.pos {
            print!(", pos: {pos}");
        }
        if tt.stop {
            print!(", stop");
        }
        println!();
    });
    let tt = Rc::new(RefCell::new(PendingTabletPadRing::default()));
    TabletPadRingSource::handle(tc, se, tt.clone(), move |tt, ev| {
        tt.borrow_mut().source = ev.source;
    });
    TabletPadRingAngle::handle(tc, se, tt.clone(), move |tt, ev| {
        tt.borrow_mut().degrees = Some(ev.degrees);
    });
    TabletPadRingStop::handle(tc, se, tt.clone(), move |tt, _| {
        tt.borrow_mut().stop = true;
    });
    let st = seat_test.clone();
    TabletPadRingFrame::handle(tc, se, tt.clone(), move |tt, ev| {
        let tt = tt.take();
        if !all && ev.seat != seat {
            return;
        }
        if all {
            print!("Seat: {}, ", st.name(ev.seat));
        }
        print!(
            "Time: {:.4}, Device: {}, Ring: {}",
            time(ev.time_usec),
            ev.input_device,
            ev.ring,
        );
        let source = match tt.source {
            1 => "finger",
            _ => "unknown",
        };
        print!(", source: {source}");
        if let Some(val) = tt.degrees {
            print!(", degrees: {val}");
        }
        if tt.stop {
            print!(", stop");
        }
        println!();
    });
    let tt = Rc::new(RefCell::new(PendingTabletPadDial::default()));
    TabletPadDialDelta::handle(tc, se, tt.clone(), move |tt, ev| {
        tt.borrow_mut().value120 = Some(ev.value120);
    });
    let st = seat_test.clone();
    TabletPadDialFrame::handle(tc, se, tt.clone(), move |tt, ev| {
        let tt = tt.take();
        if !all && ev.seat != seat {
            return;
        }
        if all {
            print!("Seat: {}, ", st.name(ev.seat));
        }
        print!(
            "Time: {:.4}, Device: {}, Dial: {}",
            time(ev.time_usec),
            ev.input_device,
            ev.dial,
        );
        if let Some(val) = tt.value120 {
            print!(", delta: {val}/120");
        }
        println!();
    });
    let st = seat_test.clone();
    TouchDown::handle(tc, se, (), move |_, ev| {
        if all || ev.seat == seat {
            if all {
                print!("Seat: {}, ", st.name(ev.seat));
            }
            println!(
                "Time: {:.4}, Touch: {}, Down: {}x{}",
                time(ev.time_usec),
                ev.id,
                ev.x,
                ev.y
            );
        }
    });
    let st = seat_test.clone();
    TouchUp::handle(tc, se, (), move |_, ev| {
        if all || ev.seat == seat {
            if all {
                print!("Seat: {}, ", st.name(ev.seat));
            }
            println!("Time: {:.4}, Touch: {}, Up", time(ev.time_usec), ev.id);
        }
    });
    let st = seat_test.clone();
    TouchMotion::handle(tc, se, (), move |_, ev| {
        if all || ev.seat == seat {
            if all {
                print!("Seat: {}, ", st.name(ev.seat));
            }
            println!(
                "Time: {:.4}, Touch: {} Motion: {}x{}",
                time(ev.time_usec),
                ev.id,
                ev.x,
                ev.y
            );
        }
    });
    let st = seat_test.clone();
    TouchCancel::handle(tc, se, (), move |_, ev| {
        if all || ev.seat == seat {
            if all {
                print!("Seat: {}, ", st.name(ev.seat));
            }
            println!("Time: {:.4}, Touch: {}, Cancel", time(ev.time_usec), ev.id);
        }
    });
    pending::<()>().await;
}

fn time(time_usec: u64) -> f64 {
    time_usec as f64 / 1_000_000f64
}

fn choose_seat(st: &SeatTest) -> u32 {
    let seat_name = match &st.args.seat {
        Some(s) => s.clone(),
        _ => {
            let mut seats: Vec<_> = st.names.borrow_mut().values().cloned().collect();
            seats.sort();
            eprintln!("Seats:");
            for seat in seats {
                eprintln!("  - {}", seat);
            }
            eprint!("Name a seat to test: ");
            let mut name = String::new();
            if let Err(e) = std::io::stdin().read_line(&mut name) {
                fatal!("Could not read from stdin: {}", e);
            }
            name
        }
    };
    let seat_name = seat_name.trim();
    for seat in st.names.borrow_mut().deref() {
        if seat.1.as_str() == seat_name {
            return *seat.0;
        }
    }
    fatal!("Unknown seat `{}`", seat_name);
}
