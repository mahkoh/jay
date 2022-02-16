use i4config::keyboard::mods::{Modifiers, ALT, CTRL, SHIFT};
use i4config::keyboard::syms::{SYM_h, SYM_j, SYM_k, SYM_l, SYM_r, SYM_t, SYM_x, SYM_y, SYM_b, SYM_Super_L};
use i4config::Direction::{Down, Left, Right, Up};
use i4config::{config, create_seat, input_devices, on_new_input_device, Command, Seat, InputDevice};
use i4config::embedded::grab_keyboard;

const MOD: Modifiers = ALT;

fn configure_seat(s: Seat) {
    log::info!("Configuring seat {:?}", s);

    let change_rate = move |delta| {
        let (rate, delay) = s.repeat_rate();
        let new_rate = rate - delta;
        let new_delay = delay + 10 * delta;
        log::info!("Changing repeat rate to {}/{}", new_rate, new_delay);
        s.set_repeat_rate(new_rate, new_delay);
    };

    s.bind(CTRL | SHIFT | SYM_l, move || change_rate(-1));
    s.bind(CTRL | SHIFT | SYM_r, move || change_rate(1));

    s.bind(MOD | SYM_h, move || s.focus(Left));
    s.bind(MOD | SYM_j, move || s.focus(Down));
    s.bind(MOD | SYM_k, move || s.focus(Up));
    s.bind(MOD | SYM_l, move || s.focus(Right));

    s.bind(MOD | SYM_t, move || {
        s.set_split(s.split().other());
    });

    s.bind(MOD | SHIFT | SYM_h, move || s.move_(Left));
    s.bind(MOD | SHIFT | SYM_j, move || s.move_(Down));
    s.bind(MOD | SHIFT | SYM_k, move || s.move_(Up));
    s.bind(MOD | SHIFT | SYM_l, move || s.move_(Right));

    s.bind(SYM_Super_L, || Command::new("alacritty").spawn());

    fn do_grab(s: Seat, grab: bool) {
        for device in s.input_devices() {
            if let InputDevice::Keyboard(kb) = device {
                log::info!("{}rabbing keyboard {:?}", if grab { "G" } else { "Ung" }, kb.0);
                grab_keyboard(kb, grab);
            }
        }
        if grab {
            s.unbind(SYM_y);
            s.bind(MOD | SYM_b, move || do_grab(s, false));
        } else {
            s.unbind(MOD | SYM_b);
            s.bind(SYM_y, move || do_grab(s, true));
        }
    }
    do_grab(s, false);
}

pub fn configure() {
    let seat = create_seat("default");
    configure_seat(seat);
    for device in input_devices() {
        device.set_seat(seat);
    }
    on_new_input_device(move |device| device.set_seat(seat));
}

config!(configure);
