use jay_config::embedded::grab_input_device;
use jay_config::keyboard::mods::{Modifiers, ALT, CTRL, SHIFT};
use jay_config::keyboard::syms::{
    SYM_Super_L, SYM_b, SYM_comma, SYM_d, SYM_f, SYM_h, SYM_j, SYM_k, SYM_l, SYM_m, SYM_p,
    SYM_period, SYM_q, SYM_r, SYM_t, SYM_v, SYM_y, SYM_F1, SYM_F10, SYM_F11, SYM_F12, SYM_F2,
    SYM_F3, SYM_F4, SYM_F5, SYM_F6, SYM_F7, SYM_F8, SYM_F9,
};
use jay_config::theme::{get_title_height, set_title_color, set_title_height, Color};
use jay_config::Axis::{Horizontal, Vertical};
use jay_config::Direction::{Down, Left, Right, Up};
use jay_config::{
    config, create_seat, input_devices, on_new_input_device, quit, switch_to_vt, Command, Seat,
};
use rand::Rng;
use jay_config::input::CAP_KEYBOARD;

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

    s.bind(MOD | SYM_comma, move || {
        let mut rng = rand::thread_rng();
        set_title_color(Color {
            r: rng.gen(),
            g: rng.gen(),
            b: rng.gen(),
            a: rng.gen(),
        })
    });

    s.bind(MOD | SYM_period, move || {
        set_title_height(get_title_height() + 1)
    });

    s.bind(MOD | SYM_h, move || s.focus(Left));
    s.bind(MOD | SYM_j, move || s.focus(Down));
    s.bind(MOD | SYM_k, move || s.focus(Up));
    s.bind(MOD | SYM_l, move || s.focus(Right));

    s.bind(MOD | SYM_d, move || s.create_split(Horizontal));
    s.bind(MOD | SYM_v, move || s.create_split(Vertical));

    s.bind(MOD | SYM_t, move || {
        s.set_split(s.split().other());
    });

    s.bind(MOD | SYM_m, move || {
        s.set_mono(!s.mono());
    });

    s.bind(MOD | SYM_f, move || {
        s.focus_parent();
    });

    s.bind(MOD | SHIFT | SYM_f, move || {
        s.toggle_floating();
    });

    s.bind(MOD | SHIFT | SYM_h, move || s.move_(Left));
    s.bind(MOD | SHIFT | SYM_j, move || s.move_(Down));
    s.bind(MOD | SHIFT | SYM_k, move || s.move_(Up));
    s.bind(MOD | SHIFT | SYM_l, move || s.move_(Right));

    s.bind(SYM_Super_L, || Command::new("alacritty").spawn());

    s.bind(MOD | SYM_p, || Command::new("xeyes").spawn());

    s.bind(MOD | SYM_q, quit);

    s.bind(CTRL | ALT | SYM_F1, || switch_to_vt(1));
    s.bind(CTRL | ALT | SYM_F2, || switch_to_vt(2));
    s.bind(CTRL | ALT | SYM_F3, || switch_to_vt(3));
    s.bind(CTRL | ALT | SYM_F4, || switch_to_vt(4));
    s.bind(CTRL | ALT | SYM_F5, || switch_to_vt(5));
    s.bind(CTRL | ALT | SYM_F6, || switch_to_vt(6));
    s.bind(CTRL | ALT | SYM_F7, || switch_to_vt(7));
    s.bind(CTRL | ALT | SYM_F8, || switch_to_vt(8));
    s.bind(CTRL | ALT | SYM_F9, || switch_to_vt(9));
    s.bind(CTRL | ALT | SYM_F10, || switch_to_vt(10));
    s.bind(CTRL | ALT | SYM_F11, || switch_to_vt(11));
    s.bind(CTRL | ALT | SYM_F12, || switch_to_vt(12));

    fn do_grab(s: Seat, grab: bool) {
        for device in s.input_devices() {
            if device.has_capability(CAP_KEYBOARD) {
                log::info!(
                    "{}rabbing keyboard {:?}",
                    if grab { "G" } else { "Ung" },
                    device.0
                );
                grab_input_device(device, grab);
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
