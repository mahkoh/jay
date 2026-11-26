use crate::it::test_error::TestErrorError;
use crate::it::test_error::TestResult;
use crate::it::testrun::TestRun;
use crate::wire::hyprland_global_shortcut_v1::*;
use jay_config::keyboard::mods::Modifiers;
use jay_config::keyboard::mods::SHIFT;
use jay_config::keyboard::syms::SYM_1;
use jay_config::keyboard::syms::SYM_KP_5;
use jay_config::keyboard::syms::SYM_KP_Begin;
use jay_config::keyboard::syms::SYM_KP_Decimal;
use jay_config::keyboard::syms::SYM_KP_Delete;
use std::cell::Cell;
use std::rc::Rc;

testcase!();

struct TestGlobalShortcut {
    pressed_count: Cell<u32>,
    released_count: Cell<u32>,
}

impl HyprlandGlobalShortcutV1EventHandler for TestGlobalShortcut {
    type Error = TestErrorError;

    fn pressed(&self, _ev: Pressed, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.pressed_count.set(self.pressed_count.get() + 1);
        Ok(())
    }

    fn released(&self, _ev: Released, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.released_count.set(self.released_count.get() + 1);
        Ok(())
    }
}

synthetic_event_handler!(TestGlobalShortcut);

const KEYMAP: &str = r#"
xkb_keymap {
    xkb_keycodes {
        <AE01> = 10;
        <LFSH> = 50;
        <NMLK> = 77;
        <KP5> = 84;
        <KP0> = 90;
        <KPDL> = 91;
    };
    xkb_types {
        virtual_modifiers NumLock;

        type "ONE_LEVEL" {
            modifiers = none;
            map[none] = Level1;
        };
        type "TWO_LEVEL" {
            modifiers = Shift;
            map[Shift] = Level2;
        };
        type "KEYPAD" {
            modifiers = Shift+NumLock;
            map[none] = Level1;
            map[Shift] = Level1;
            preserve[Shift] = Shift;
            map[NumLock] = Level2;
            map[Shift+NumLock] = Level2;
            preserve[Shift+NumLock] = Shift;
        };
    };
    xkb_compatibility {
        virtual_modifiers NumLock;
        interpret.repeat = False;
        interpret Num_Lock+Any {
            virtualModifier = NumLock;
            action = LockMods(modifiers=NumLock);
        };
        interpret Any+Any {
            action = SetMods(modifiers=modMapMods);
        };
    };
    xkb_symbols {
        key <AE01> { type[Group1]="TWO_LEVEL", [ 1, exclam ] };
        key <LFSH> { type[Group1]="ONE_LEVEL", [ Shift_L ] };
        key <NMLK> { type[Group1]="ONE_LEVEL", [ Num_Lock ] };
        key <KP5>  { type[Group1]="KEYPAD", [ KP_Begin, KP_5 ] };
        key <KP0>  { type[Group1]="KEYPAD", [ KP_Insert, KP_0 ] };
        key <KPDL> { type[Group1]="KEYPAD", [ KP_Delete, KP_Decimal ] };
        modifier_map Shift { <LFSH> };
        modifier_map Mod2 { <NMLK> };
    };
};
"#;

async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;
    let keymap = run.cfg.parse_keymap(KEYMAP)?;
    run.cfg.set_keymap(ds.seat.id(), keymap)?;

    run.cfg.add_shortcut(ds.seat.id(), SYM_KP_Begin)?;
    run.cfg.add_shortcut(ds.seat.id(), SYM_KP_Delete)?;
    run.cfg.add_shortcut(ds.seat.id(), SYM_KP_5)?;
    run.cfg.add_shortcut(ds.seat.id(), SYM_KP_Decimal)?;
    run.cfg.add_shortcut(ds.seat.id(), SHIFT | SYM_KP_5)?;
    run.cfg.add_shortcut(ds.seat.id(), SHIFT | SYM_1)?;

    let client = run.create_client()?;

    let mute_sc = Rc::new(TestGlobalShortcut {
        pressed_count: Cell::new(0),
        released_count: Cell::new(0),
    });
    let mute_id = client.send_hyprland_global_shortcuts_manager_v1_register_shortcut(
        "discord-toggle-mute",
        "com.discordapp.Discord",
        "Toggle Mute",
        "KP_5",
    );
    client.set_synthetic_event_handler(mute_id, &mute_sc);

    let deafen_sc = Rc::new(TestGlobalShortcut {
        pressed_count: Cell::new(0),
        released_count: Cell::new(0),
    });
    let deafen_id = client.send_hyprland_global_shortcuts_manager_v1_register_shortcut(
        "discord-toggle-deafen",
        "com.discordapp.Discord",
        "Toggle Deafen",
        "KP_0",
    );
    client.set_synthetic_event_handler(deafen_id, &deafen_sc);
    run.sync().await;

    run.state.global_shortcuts.set_configured_trigger(
        "com.discordapp.Discord",
        "discord-toggle-deafen",
        Modifiers::NONE,
        SYM_KP_Decimal,
        "KP_Decimal",
    );
    run.sync().await;

    // NumLock OFF: keypad resolves to Level 0 (`KP_Begin` / `KP_Delete`).
    {
        let _kp5 = ds.kb.press(76);
        run.sync().await;
    }
    run.sync().await;
    tassert!(
        run.cfg
            .invoked_shortcuts
            .contains(&(ds.seat.id(), SYM_KP_Begin.into()))
    );
    tassert!(
        !run.cfg
            .invoked_shortcuts
            .contains(&(ds.seat.id(), SYM_KP_5.into()))
    );
    tassert_eq!(mute_sc.pressed_count.get(), 0);

    run.cfg.invoked_shortcuts.clear();
    {
        let _kpdl = ds.kb.press(83);
        run.sync().await;
    }
    run.sync().await;
    tassert!(
        run.cfg
            .invoked_shortcuts
            .contains(&(ds.seat.id(), SYM_KP_Delete.into()))
    );
    tassert_eq!(deafen_sc.pressed_count.get(), 0);

    // Turn NumLock ON (`KEY_NUMLOCK` = 69).
    {
        let _nmlk = ds.kb.press(69);
        run.sync().await;
    }
    run.sync().await;

    run.cfg.invoked_shortcuts.clear();
    {
        let _kp5 = ds.kb.press(76);
        run.sync().await;
    }
    run.sync().await;
    tassert!(
        run.cfg
            .invoked_shortcuts
            .contains(&(ds.seat.id(), SYM_KP_5.into()))
    );
    tassert_eq!(mute_sc.pressed_count.get(), 1);
    tassert_eq!(mute_sc.released_count.get(), 1);

    run.cfg.invoked_shortcuts.clear();
    {
        let _kpdl = ds.kb.press(83);
        run.sync().await;
    }
    run.sync().await;
    tassert!(
        run.cfg
            .invoked_shortcuts
            .contains(&(ds.seat.id(), SYM_KP_Decimal.into()))
    );
    tassert_eq!(deafen_sc.pressed_count.get(), 1);
    tassert_eq!(deafen_sc.released_count.get(), 1);

    // Configured trigger `KP_Decimal` overrides preferred trigger `KP_0`.
    {
        let _kp0 = ds.kb.press(82);
        run.sync().await;
    }
    run.sync().await;
    tassert_eq!(deafen_sc.pressed_count.get(), 1);

    // Shift + KP_5 and Shift + 1 both keep non-NumLock level lookup intact.
    run.cfg.invoked_shortcuts.clear();
    {
        let _shift = ds.kb.press(42);
        let _kp5 = ds.kb.press(76);
        run.sync().await;
    }
    run.sync().await;
    tassert!(
        run.cfg
            .invoked_shortcuts
            .contains(&(ds.seat.id(), SHIFT | SYM_KP_5))
    );

    run.cfg.invoked_shortcuts.clear();
    {
        let _shift = ds.kb.press(42);
        let _one = ds.kb.press(2);
        run.sync().await;
    }
    run.sync().await;
    tassert!(
        run.cfg
            .invoked_shortcuts
            .contains(&(ds.seat.id(), SHIFT | SYM_1))
    );

    Ok(())
}
