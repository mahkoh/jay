use {
    crate::{
        cli::{GlobalArgs, SeatTestArgs},
        ifs::wl_seat::wl_pointer::{PendingScroll, CONTINUOUS, FINGER, WHEEL},
        tools::tool_client::{with_tool_client, Handle, ToolClient},
        wire::{
            jay_compositor::{GetSeats, Seat, SeatEvents},
            jay_seat_events::{
                Axis120, AxisFrame, AxisPx, AxisSource, AxisStop, Button, Key, Modifiers,
                PointerAbs, PointerRel,
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
        let px_x = ps.px[0].take();
        let px_y = ps.px[1].take();
        let stop_x = ps.stop[0].take();
        let stop_y = ps.stop[1].take();
        let v120_x = ps.v120[0].take();
        let v120_y = ps.v120[1].take();
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
            for (axis, px, steps, stop) in [
                ("horizontal", px_x, v120_x, stop_x),
                ("vertical", px_y, v120_y, stop_y),
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
            }
            println!();
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
