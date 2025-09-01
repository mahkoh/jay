use {
    crate::{
        it::{test_error::TestResult, testrun::TestRun},
        theme::Color,
        wire::WlBufferId,
    },
    std::rc::Rc,
};

testcase!();

/// Test wl_buffer.release event functionality
async fn test(run: Rc<TestRun>) -> TestResult {
    let client = run.create_client().await?;

    // Create a surface and buffer
    let surface = client.comp.create_surface().await?;
    let buffer1 = client.spbm.create_buffer(Color::from_srgb(255, 0, 0))?;
    let buffer2 = client.spbm.create_buffer(Color::from_srgb(0, 255, 0))?;

    // Initially both buffers should be marked as released (not in use)
    tassert!(buffer1.released.get());
    tassert!(buffer2.released.get());

    // Attach the first buffer and commit
    surface.attach(buffer1.id)?;
    surface.commit()?;

    // The buffer should now be in use, so released should be false
    buffer1.released.set(false); // Reset to track actual release event

    client.sync().await;

    // Attach a different buffer and commit - this should cause the first buffer to be released
    surface.attach(buffer2.id)?;
    surface.commit()?;

    buffer2.released.set(false); // Reset to track release of second buffer

    client.sync().await;

    // Buffer1 should now be released since buffer2 is attached
    tassert!(buffer1.released.get());

    // Create a third buffer and attach it
    let buffer3 = client.spbm.create_buffer(Color::from_srgb(0, 0, 255))?;
    surface.attach(buffer3.id)?;
    surface.commit()?;

    buffer3.released.set(false);

    client.sync().await;

    // Buffer2 should now be released since buffer3 is attached
    tassert!(buffer2.released.get());

    // Test buffer reuse - we should be able to reuse buffer1 now that it's released
    surface.attach(buffer1.id)?;
    surface.commit()?;

    buffer1.released.set(false);

    client.sync().await;

    // Buffer3 should now be released since buffer1 is attached again
    tassert!(buffer3.released.get());

    // Finally, detach the buffer (attach NULL) - this should release buffer1
    surface.attach(WlBufferId::NONE)?;
    surface.commit()?;

    client.sync().await;

    // Buffer1 should now be released since no buffer is attached
    tassert!(buffer1.released.get());

    Ok(())
}
