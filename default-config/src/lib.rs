use {
    chrono::{format::StrftimeItems, Local, Timelike},
    jay_config::{
        config,
        drm::{get_connector, on_connector_connected, on_graphics_initialized, on_new_connector},
        embedded::grab_input_device,
        get_timer, get_workspace,
        input::{
            capability::{CAP_KEYBOARD, CAP_POINTER},
            create_seat, input_devices, on_new_input_device, InputDevice, Seat,
        },
        keyboard::{
            mods::{Modifiers, ALT, CTRL, SHIFT},
            syms::{
                SYM_Super_L, SYM_a, SYM_b, SYM_c, SYM_d, SYM_e, SYM_f, SYM_h, SYM_j, SYM_k, SYM_l,
                SYM_m, SYM_o, SYM_p, SYM_q, SYM_t, SYM_u, SYM_v, SYM_y, SYM_F1, SYM_F10, SYM_F11,
                SYM_F12, SYM_F13, SYM_F14, SYM_F15, SYM_F16, SYM_F17, SYM_F18, SYM_F19, SYM_F2,
                SYM_F20, SYM_F21, SYM_F22, SYM_F23, SYM_F24, SYM_F25, SYM_F3, SYM_F4, SYM_F5,
                SYM_F6, SYM_F7, SYM_F8, SYM_F9,
            },
        },
        quit, set_env,
        status::set_status,
        switch_to_vt,
        Axis::{Horizontal, Vertical},
        Command,
        Direction::{Down, Left, Right, Up},
    },
    std::time::Duration,
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

    s.bind(MOD | SYM_u, move || s.toggle_fullscreen());

    s.bind(MOD | SHIFT | SYM_c, move || s.close());

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
        s.bind(MOD | SHIFT | sym, move || s.set_workspace(ws));
    }

    s.bind(MOD | SYM_a, || {
        Command::new("spotify-remote").arg("a").spawn()
    });
    s.bind(MOD | SYM_o, || {
        Command::new("spotify-remote").arg("o").spawn()
    });
    s.bind(MOD | SYM_e, || {
        Command::new("spotify-remote").arg("e").spawn()
    });

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

    let handle_connectors_changed = || {
        let left = get_connector("HDMI-A-1");
        let right = get_connector("DP-1");
        if left.connected() && right.connected() {
            left.set_position(0, 0);
            right.set_position(left.width(), 0);
        }
    };
    on_new_connector(move |_| handle_connectors_changed());
    on_connector_connected(move |_| handle_connectors_changed());
    handle_connectors_changed();

    {
        let time_format: Vec<_> = StrftimeItems::new("%Y-%m-%d %H:%M:%S").collect();
        let update_status = move || {
            let status = format!("{}", Local::now().format_with_items(time_format.iter()),);
            set_status(&status);
        };
        update_status();
        let initial = {
            let now = Local::now();
            5000 - (now.second() * 1000 + now.timestamp_subsec_millis()) % 5000
        };
        let timer = get_timer("status_timer");
        timer.program(
            Duration::from_millis(initial as u64),
            Some(Duration::from_secs(5)),
        );
        timer.on_tick(update_status);
    }

    set_env("GTK_THEME", "Adwaita:dark");

    on_graphics_initialized(|| {
        Command::new("mako").spawn();
    });
}

config!(configure);
