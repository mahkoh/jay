use {
    crate::it::{test_error::TestResult, testrun::TestRun},
    jay_config::keyboard::syms::SYM_F13,
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    run.cfg.add_shortcut(ds.seat.id(), SYM_F13)?;
    run.sync().await;

    ds.kb.press(1);
    run.sync().await;
    tassert!(run.cfg.invoked_shortcuts.is_empty());

    let keymap = r#"
xkb_keymap {
    xkb_keycodes {
          <1> = 9; # ESC
    };
    xkb_types {
    };
    xkb_compatibility {
    };
    xkb_symbols {
        key <1> { [ F13 ] };
    };
};
    "#;

    let keymap = run.cfg.parse_keymap(keymap)?;
    run.cfg.set_keymap(ds.seat.id(), keymap)?;
    run.sync().await;

    ds.kb.press(1);
    run.sync().await;
    tassert!(
        run.cfg
            .invoked_shortcuts
            .contains(&(ds.seat.id(), SYM_F13.into()))
    );

    Ok(())
}
