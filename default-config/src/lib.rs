use jay_config::embedded::grab_input_device;
use jay_config::input::capability::{CAP_KEYBOARD, CAP_POINTER};
use jay_config::input::InputDevice;
use jay_config::keyboard::mods::{Modifiers, ALT, CTRL, SHIFT};
use jay_config::keyboard::syms::{
    SYM_Super_L, SYM_b, SYM_d, SYM_f, SYM_h, SYM_j, SYM_k, SYM_l, SYM_m, SYM_p, SYM_q, SYM_t,
    SYM_v, SYM_y, SYM_F1, SYM_F10, SYM_F11, SYM_F12, SYM_F13, SYM_F14, SYM_F15, SYM_F16, SYM_F17,
    SYM_F18, SYM_F19, SYM_F2, SYM_F20, SYM_F21, SYM_F22, SYM_F23, SYM_F24, SYM_F25, SYM_F3, SYM_F4,
    SYM_F5, SYM_F6, SYM_F7, SYM_F8, SYM_F9,
};
use jay_config::Axis::{Horizontal, Vertical};
use jay_config::Direction::{Down, Left, Right, Up};
use jay_config::{
    config, create_seat, get_workspace, input_devices, on_new_input_device, quit, switch_to_vt,
    Command, Seat,
};

const MOD: Modifiers = ALT;

fn configure_seat(s: Seat) {
    s.bind(MOD | SYM_h, move || s.focus(Left));
    s.bind(MOD | SYM_j, move || s.focus(Down));
    s.bind(MOD | SYM_k, move || s.focus(Up));
    s.bind(MOD | SYM_l, move || s.focus(Right));

    s.bind(MOD | SHIFT | SYM_h, move || s.move_(Left));
    s.bind(MOD | SHIFT | SYM_j, move || s.move_(Down));
    s.bind(MOD | SHIFT | SYM_k, move || s.move_(Up));
    s.bind(MOD | SHIFT | SYM_l, move || s.move_(Right));

    s.bind(MOD | SYM_d, move || s.create_split(Horizontal));
    s.bind(MOD | SYM_v, move || s.create_split(Vertical));

    s.bind(MOD | SYM_t, move || s.set_split(s.split().other()));

    s.bind(MOD | SYM_m, move || s.set_mono(!s.mono()));

    s.bind(MOD | SYM_f, move || s.focus_parent());

    s.bind(MOD | SHIFT | SYM_f, move || s.toggle_floating());

    s.bind(SYM_Super_L, || Command::new("alacritty").spawn());

    s.bind(MOD | SYM_p, || Command::new("bemenu-run").spawn());

    s.bind(MOD | SYM_q, quit);

    let fnkeys = [
        SYM_F1, SYM_F2, SYM_F3, SYM_F4, SYM_F5, SYM_F6, SYM_F7, SYM_F8, SYM_F9, SYM_F10, SYM_F11,
        SYM_F12,
    ];
    for (i, sym) in fnkeys.into_iter().enumerate() {
        s.bind(CTRL | ALT | sym, move || switch_to_vt(i as u32 + 1));
    }

    let fnkeys2 = [
        SYM_F13, SYM_F14, SYM_F15, SYM_F16, SYM_F17, SYM_F18, SYM_F19, SYM_F20, SYM_F21, SYM_F22,
        SYM_F23, SYM_F24, SYM_F25,
    ];
    for (i, sym) in fnkeys2.into_iter().enumerate() {
        let ws = get_workspace(&format!("{}", i + 1));
        s.bind(MOD | sym, move || s.show_workspace(ws));
    }

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
    let handle_input_device = move |device: InputDevice| {
        if device.has_capability(CAP_POINTER) {
            device.set_left_handed(true);
            device.set_transform_matrix([[0.35, 0.0], [0.0, 0.35]]);
        }
        device.set_seat(seat);
    };
    input_devices().into_iter().for_each(handle_input_device);
    on_new_input_device(handle_input_device);
}

config!(configure);
