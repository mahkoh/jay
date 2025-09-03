use {
    crate::it::{test_error::TestResult, testrun::TestRun},
    std::rc::Rc,
};

testcase!();

/// Test that committing damage on a visible surface causes the backend connector to be damaged.
/// This test verifies that surface damage triggers backend connector damage tracking AND
/// that the frontend actually calls into the backend connector's damage method,
/// ensuring the rendering pipeline knows when to update the display.
async fn test(run: Rc<TestRun>) -> TestResult {
    run.backend.install_default()?;
    let client = run.create_client().await?;

    // Get connector for tracking backend damage state
    let connector_id = run.backend.default_connector.id;
    let connector_data = run.state.connectors.get(&connector_id).unwrap();

    // Create a visible window with mapped surface
    let window = client.create_window().await?;
    let buffer = client
        .spbm
        .create_buffer(crate::theme::Color::from_srgb(0, 255, 0))?;
    window.surface.attach(buffer.id)?;
    window.map().await?;
    client.sync().await;

    // Test 1: Ensure initially the backend is not damaged
    connector_data.damaged.set(false);
    run.backend.default_connector.damage_calls.set(0);
    tassert!(!connector_data.damaged.get());
    tassert_eq!(run.backend.default_connector.damage_calls.get(), 0);

    // Test 2: Add surface damage and commit - this should trigger backend connector damage
    window.surface.damage(10, 10, 50, 50)?;
    window.surface.commit()?;
    client.sync().await;

    // Critical test: Verify the backend connector is now damaged AND the backend method was called
    tassert!(connector_data.damaged.get());
    tassert!(run.backend.default_connector.damage_calls.get() > 0);

    // Test 3: Reset damage state and test buffer damage
    connector_data.damaged.set(false);
    let previous_calls = run.backend.default_connector.damage_calls.get();
    tassert!(!connector_data.damaged.get());

    // Add buffer damage and commit - this should also trigger backend connector damage
    window.surface.damage_buffer(0, 0, 1, 1)?; // Damage entire 1x1 buffer
    window.surface.commit()?;
    client.sync().await;

    // Verify the backend connector is damaged again AND more backend calls were made
    tassert!(connector_data.damaged.get());
    tassert!(run.backend.default_connector.damage_calls.get() > previous_calls);

    // Test 4: Test that invisible surfaces do not trigger backend damage
    let invisible_surface = client.comp.create_surface().await?;
    let invisible_buffer = client
        .spbm
        .create_buffer(crate::theme::Color::from_srgb(255, 255, 0))?;
    invisible_surface.attach(invisible_buffer.id)?;
    invisible_surface.commit()?; // Initial commit to attach buffer
    client.sync().await;

    // Reset damage state
    connector_data.damaged.set(false);
    let invisible_calls_before = run.backend.default_connector.damage_calls.get();
    tassert!(!connector_data.damaged.get());

    // Add damage to invisible surface and commit
    invisible_surface.damage(20, 20, 30, 30)?;
    invisible_surface.commit()?;
    client.sync().await;

    // Invisible surface damage should NOT trigger backend connector damage or backend calls
    tassert!(!connector_data.damaged.get());
    tassert_eq!(
        run.backend.default_connector.damage_calls.get(),
        invisible_calls_before
    );

    // Test 5: Test multiple damage areas on visible surface
    connector_data.damaged.set(false);
    let multi_calls_before = run.backend.default_connector.damage_calls.get();
    tassert!(!connector_data.damaged.get());

    // Add multiple damage rectangles to visible surface
    window.surface.damage(5, 5, 10, 10)?;
    window.surface.damage(25, 25, 15, 15)?;
    window.surface.damage_buffer(0, 0, 1, 1)?;
    window.surface.commit()?;
    client.sync().await;

    // Multiple damage areas on visible surface should trigger backend connector damage and calls
    tassert!(connector_data.damaged.get());
    tassert!(run.backend.default_connector.damage_calls.get() > multi_calls_before);

    // Test 6: Test that damage without commit does not trigger backend damage
    connector_data.damaged.set(false);
    let no_commit_calls_before = run.backend.default_connector.damage_calls.get();
    tassert!(!connector_data.damaged.get());

    // Add damage but don't commit
    window.surface.damage(40, 40, 20, 20)?;
    client.sync().await;

    // Damage without commit should NOT trigger backend connector damage or backend calls
    tassert!(!connector_data.damaged.get());
    tassert_eq!(
        run.backend.default_connector.damage_calls.get(),
        no_commit_calls_before
    );

    // Now commit the pending damage
    window.surface.commit()?;
    client.sync().await;

    // After commit, backend connector should be damaged and backend called
    tassert!(connector_data.damaged.get());
    tassert!(run.backend.default_connector.damage_calls.get() > no_commit_calls_before);

    Ok(())
}
