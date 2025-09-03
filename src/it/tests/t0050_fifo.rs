use {
    crate::{
        it::{
            test_error::{TestError, TestResult},
            testrun::TestRun,
        },
        theme::Color,
    },
    std::rc::Rc,
};

testcase!();

/// Test wp_fifo_v1 protocol implementation
/// This test verifies that the compositor correctly implements the fifo-v1 protocol
/// for synchronizing surface updates with display refresh cycles.
async fn test(run: Rc<TestRun>) -> TestResult {
    run.backend.install_default()?;
    let client = run.create_client().await?;

    // Create a visible window so fifo constraints can be tested
    let window = client.create_window().await?;
    window.map().await?;
    client.sync().await;

    let fifo_manager = &client.fifo_manager;
    let surface = &window.surface.surface;
    let connector_id = run.backend.default_connector.id;

    // Get a fifo object for the surface
    let fifo = fifo_manager.get_fifo(surface)?;
    client.sync().await;

    // Test 1: Basic fifo barrier functionality without wait_barrier
    // This should not block the commit - the old buffer should be released immediately
    fifo.set_barrier()?;
    let buffer1 = client.spbm.create_buffer(Color::from_srgb(255, 0, 0))?;
    surface.attach(buffer1.id)?;
    surface.commit()?;
    client.sync().await;

    // Test 1a: Critical test - without wait_barrier, commit should be applied immediately
    let buffer1a = client.spbm.create_buffer(Color::from_srgb(255, 128, 0))?;

    // Reset buffer tracking to detect when buffer1 gets released
    buffer1.released.set(false);

    // Attach new buffer and commit WITHOUT wait_barrier
    surface.attach(buffer1a.id)?;
    surface.commit()?;
    client.sync().await;

    // WITHOUT wait_barrier, the commit should be applied immediately even if barrier is set
    // So buffer1 should be released immediately, without needing a vblank
    if !buffer1.released.get() {
        return Err(TestError::new(
            "Without wait_barrier, commit should be applied immediately - buffer1 was not released",
        ));
    }

    // Test 2: Critical test - wait_barrier SHOULD block commit until next after_latch
    // This contrasts with Test 1a above where commits were applied immediately

    // Set the barrier and immediately test wait_barrier (without intermediate commits that might clear it)
    fifo.set_barrier()?;
    fifo.wait_barrier()?;

    let buffer2_current = client.spbm.create_buffer(Color::from_srgb(0, 255, 0))?;
    let buffer2_next = client.spbm.create_buffer(Color::from_srgb(0, 0, 255))?;

    // First attach current buffer
    surface.attach(buffer2_current.id)?;
    surface.commit()?;
    client.sync().await;

    // Reset tracking for the buffer we want to monitor
    buffer2_current.released.set(false);

    // Now attach the new buffer - this should trigger the wait_barrier blocking
    surface.attach(buffer2_next.id)?;
    surface.commit()?;
    client.sync().await;

    // CRITICAL: The commit should be blocked, so buffer2_current should NOT be released yet
    if buffer2_current.released.get() {
        return Err(TestError::new(
            "wait_barrier did not block the commit - buffer2_current was released immediately",
        ));
    }

    // The commit was successfully blocked! This proves wait_barrier works.
    // Now trigger after_latch to clear the barrier and apply the queued commit
    run.state.latch(connector_id);
    client.sync().await;

    // After after_latch, the barrier should be cleared and the commit applied
    if !buffer2_current.released.get() {
        return Err(TestError::new(
            "after_latch should have cleared barrier and applied commit - buffer2_current was not released",
        ));
    }

    // Test 3: Test tearing mode - barrier should be cleared on vblank instead of immediately
    fifo.set_barrier()?;
    fifo.wait_barrier()?;

    let buffer3_current = client.spbm.create_buffer(Color::from_srgb(128, 128, 0))?;
    let buffer3_next = client.spbm.create_buffer(Color::from_srgb(0, 128, 128))?;

    // First attach current buffer
    surface.attach(buffer3_current.id)?;
    surface.commit()?;
    client.sync().await;

    // Reset tracking for the buffer we want to monitor
    buffer3_current.released.set(false);

    // Now attach the new buffer - this should trigger the wait_barrier blocking
    surface.attach(buffer3_next.id)?;
    surface.commit()?;
    client.sync().await;

    // Verify the commit is blocked
    if buffer3_current.released.get() {
        return Err(TestError::new(
            "wait_barrier did not block the commit in tearing test - buffer3_current was released immediately",
        ));
    }

    // Trigger latch with tearing=true - this should defer clearing to vblank
    run.state.latch_tearing(connector_id);
    client.sync().await;

    // With tearing=true, the barrier should NOT be cleared yet, commit should still be blocked
    if buffer3_current.released.get() {
        return Err(TestError::new(
            "In tearing mode, latch should not clear barrier immediately - buffer3_current was released",
        ));
    }

    // Now trigger vblank - this should clear the barrier and apply the commit
    run.state.vblank(connector_id);
    client.sync().await;

    // After vblank, the commit should be applied
    if !buffer3_current.released.get() {
        return Err(TestError::new(
            "vblank should have cleared barrier and applied commit - buffer3_current was not released",
        ));
    }

    Ok(())
}
