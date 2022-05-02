use {
    chrono::{format::StrftimeItems, Local, Timelike},
    jay_config::{
        config,
        drm::on_graphics_initialized,
        get_timer, get_workspace,
        input::{get_seat, input_devices, on_new_input_device, InputDevice, Seat},
        keyboard::{
            mods::{Modifiers, ALT, CTRL, SHIFT},
            syms::{
                SYM_Super_L, SYM_c, SYM_d, SYM_f, SYM_h, SYM_j, SYM_k, SYM_l, SYM_m, SYM_p, SYM_q,
                SYM_r, SYM_t, SYM_u, SYM_v, SYM_F1, SYM_F10, SYM_F11, SYM_F12, SYM_F13, SYM_F14,
                SYM_F15, SYM_F16, SYM_F17, SYM_F18, SYM_F19, SYM_F2, SYM_F20, SYM_F21, SYM_F22,
                SYM_F23, SYM_F24, SYM_F25, SYM_F3, SYM_F4, SYM_F5, SYM_F6, SYM_F7, SYM_F8, SYM_F9,
            },
        },
        quit, reload,
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

    s.bind(MOD | SYM_t, move || s.toggle_split());
    s.bind(MOD | SYM_m, move || s.toggle_mono());
    s.bind(MOD | SYM_u, move || s.toggle_fullscreen());

    s.bind(MOD | SYM_f, move || s.focus_parent());

    s.bind(MOD | SHIFT | SYM_c, move || s.close());

    s.bind(MOD | SHIFT | SYM_f, move || s.toggle_floating());

    s.bind(SYM_Super_L, || Command::new("alacritty").spawn());

    s.bind(MOD | SYM_p, || Command::new("bemenu-run").spawn());

    s.bind(MOD | SYM_q, quit);

    s.bind(MOD | SHIFT | SYM_r, reload);

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
}

pub fn configure() {
    let seat = get_seat("default");
    configure_seat(seat);
    let handle_input_device = move |device: InputDevice| {
        device.set_seat(seat);
    };
    input_devices().into_iter().for_each(handle_input_device);
    on_new_input_device(handle_input_device);

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

    on_graphics_initialized(|| {
        Command::new("mako").spawn();
    });
}

config!(configure);
