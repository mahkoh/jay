use {
    crate::it::{test_error::TestResult, testrun::TestRun},
    std::rc::Rc,
};

testcase!();

/// Test wl_surface.frame callbacks
/// This test verifies that the compositor sends wl_callback.done events after the vblank event.
/// According to the Wayland protocol, frame callbacks should be fired to indicate when
/// it's a good time to start drawing the next frame, and they should be posted after
/// the compositor has finished presenting the previous frame (i.e., after vblank).
async fn test(run: Rc<TestRun>) -> TestResult {
    run.backend.install_default()?;
    let client = run.create_client().await?;

    // Create a visible window so frame callbacks can be triggered
    let window = client.create_window().await?;
    window.map().await?;
    client.sync().await;

    // Test 1: Basic frame callback functionality
    let surface = &window.surface.surface;
    let callback1 = surface.frame()?;
    surface.commit()?;
    client.sync().await;

    // Manually trigger vblank event to simulate frame completion
    let connector_id = run.backend.default_connector.id;

    // Trigger vblank manually - this processes frame callbacks
    run.state.vblank(connector_id);
    client.sync().await;

    // The frame callback should have been fired after vblank
    tassert!(callback1.done.get());

    // Test 2: Multiple frame callbacks
    let callback2 = surface.frame()?;
    let callback3 = surface.frame()?;
    surface.commit()?;
    client.sync().await;

    // Before triggering vblank, callbacks should not be done yet
    tassert!(!callback2.done.get());
    tassert!(!callback3.done.get());

    // Trigger vblank manually - this processes frame callbacks
    run.state.vblank(connector_id);
    client.sync().await;

    // Both callbacks should be done after vblank
    tassert!(callback2.done.get());
    tassert!(callback3.done.get());

    // Test 3: Frame callbacks on invisible surface should not be processed
    // Create a new surface but don't make it visible
    let invisible_surface = client.comp.create_surface().await?;
    let buffer = client
        .spbm
        .create_buffer(crate::theme::Color::from_srgb(255, 0, 0))?;
    invisible_surface.attach(buffer.id)?;

    let callback_invisible = invisible_surface.frame()?;
    invisible_surface.commit()?;
    client.sync().await;

    // Trigger vblank manually - this processes frame callbacks
    run.state.vblank(connector_id);
    client.sync().await;

    // Frame callback on invisible surface should not be processed
    tassert!(!callback_invisible.done.get());

    // Test 4: Frame callback timing - verify they happen after vblank
    let callback_timing = surface.frame()?;
    surface.commit()?;
    client.sync().await;

    // The callback should not be done immediately after commit
    tassert!(!callback_timing.done.get());

    // Trigger vblank manually - this processes frame callbacks
    run.state.vblank(connector_id);
    client.sync().await;

    // Now it should be done
    tassert!(callback_timing.done.get());

    Ok(())
}
