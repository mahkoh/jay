use {
    crate::{
        it::{test_error::TestResult, testrun::TestRun},
        rect::Rect,
    },
    std::rc::Rc,
};

testcase!();

/// Test wl_surface.damage and wl_surface.damage_buffer requests
/// This test verifies that the compositor correctly handles damage requests according to the Wayland protocol
/// and creates appropriate output damage when surface damage is committed.
async fn test(run: Rc<TestRun>) -> TestResult {
    run.backend.install_default()?;
    let client = run.create_client().await?;

    // Get connector for tracking output damage
    let connector_id = run.backend.default_connector.id;
    let connector_data = run.state.connectors.get(&connector_id).unwrap();

    // Create a simple surface with a buffer
    let surface = client.comp.create_surface().await?;
    let buffer = client
        .spbm
        .create_buffer(crate::theme::Color::from_srgb(255, 0, 0))?;
    surface.attach(buffer.id)?;
    surface.commit()?; // Initial commit to attach buffer
    client.sync().await;

    // Test 1: wl_surface.damage - basic functionality and damage clearing
    surface.damage(10, 10, 50, 50)?;
    client.sync().await;

    // Verify damage is pending
    {
        let (surface_damage, buffer_damage, damage_full) = surface.server.get_pending_damage();
        tassert_eq!(surface_damage.len(), 1);
        tassert_eq!(surface_damage[0], Rect::new_sized(10, 10, 50, 50).unwrap());
        tassert!(buffer_damage.is_empty());
        tassert!(!damage_full);
    }

    // Critical test: Commit should clear pending damage
    surface.commit()?;
    client.sync().await;

    {
        let (surface_damage, buffer_damage, damage_full) = surface.server.get_pending_damage();
        tassert!(surface_damage.is_empty());
        tassert!(buffer_damage.is_empty());
        tassert!(!damage_full);
    }

    // Test 2: wl_surface.damage_buffer functionality
    surface.damage_buffer(20, 20, 30, 30)?;
    client.sync().await;

    // Verify buffer damage is pending
    {
        let (surface_damage, buffer_damage, damage_full) = surface.server.get_pending_damage();
        tassert!(surface_damage.is_empty());
        tassert_eq!(buffer_damage.len(), 1);
        tassert_eq!(buffer_damage[0], Rect::new_sized(20, 20, 30, 30).unwrap());
        tassert!(!damage_full);
    }

    // Commit should clear pending buffer damage
    surface.commit()?;
    client.sync().await;

    {
        let (surface_damage, buffer_damage, damage_full) = surface.server.get_pending_damage();
        tassert!(surface_damage.is_empty());
        tassert!(buffer_damage.is_empty());
        tassert!(!damage_full);
    }

    // Test 3: Mixed surface and buffer damage
    surface.damage(5, 5, 10, 10)?;
    surface.damage_buffer(15, 15, 10, 10)?;
    surface.damage(25, 25, 10, 10)?;
    client.sync().await;

    {
        let (surface_damage, buffer_damage, damage_full) = surface.server.get_pending_damage();
        tassert_eq!(surface_damage.len(), 2);
        tassert_eq!(buffer_damage.len(), 1);
        tassert!(!damage_full);
    }

    surface.commit()?;
    client.sync().await;

    {
        let (surface_damage, buffer_damage, damage_full) = surface.server.get_pending_damage();
        tassert!(surface_damage.is_empty());
        tassert!(buffer_damage.is_empty());
        tassert!(!damage_full);
    }

    // Test 4: damage_full optimization - many small damage rects should trigger damage_full
    for i in 0..40 {
        // More than MAX_DAMAGE (32) to trigger damage_full
        surface.damage(i * 2, i * 2, 1, 1)?;
    }
    client.sync().await;

    {
        let (_surface_damage, _buffer_damage, damage_full) = surface.server.get_pending_damage();
        tassert!(damage_full); // Should have triggered damage_full optimization
    }

    // Critical: damage_full should still clear pending damage after commit
    surface.commit()?;
    client.sync().await;

    {
        let (surface_damage, buffer_damage, damage_full) = surface.server.get_pending_damage();
        tassert!(surface_damage.is_empty());
        tassert!(buffer_damage.is_empty());
        tassert!(!damage_full);
    }

    // Test 5: Verify output damage creation and values
    // For this test we need a visible surface to generate actual output damage
    let window = client.create_window().await?;
    window.surface.attach(buffer.id)?;
    window.map().await?;
    client.sync().await;

    // Get the surface's absolute position for damage calculation
    let surface_pos = window.surface.server.buffer_abs_pos.get();

    // Clear any existing output damage
    connector_data.damage.borrow_mut().clear();

    // Add specific damage and commit
    let client_damage = Rect::new_sized(10, 10, 20, 20).unwrap();
    window.surface.damage(
        client_damage.x1(),
        client_damage.y1(),
        client_damage.width(),
        client_damage.height(),
    )?;
    window.surface.commit()?;
    client.sync().await;

    // Verify output damage was created with exact correct values
    {
        let output_damage = connector_data.damage.borrow();
        tassert!(!output_damage.is_empty());

        // The surface damage should be transformed to output coordinates
        // Surface damage is moved by the surface's absolute position
        let expected_damage = client_damage.move_(surface_pos.x1(), surface_pos.y1());

        // Verify the exact output damage coordinates
        let mut found_exact_damage = false;
        for &actual_damage in output_damage.iter() {
            // Check if this output damage exactly matches our expected damage
            if actual_damage.x1() == expected_damage.x1()
                && actual_damage.y1() == expected_damage.y1()
                && actual_damage.x2() == expected_damage.x2()
                && actual_damage.y2() == expected_damage.y2()
            {
                found_exact_damage = true;
                break;
            }
        }

        if !found_exact_damage {
            // If exact match not found, provide detailed debugging info
            run.errors.push(format!(
                "Expected output damage: x1={}, y1={}, x2={}, y2={} ({}x{})",
                expected_damage.x1(),
                expected_damage.y1(),
                expected_damage.x2(),
                expected_damage.y2(),
                expected_damage.width(),
                expected_damage.height()
            ));
            run.errors.push(format!(
                "Surface position: x1={}, y1={}",
                surface_pos.x1(),
                surface_pos.y1()
            ));
            run.errors.push(format!(
                "Client damage: x1={}, y1={}, x2={}, y2={} ({}x{})",
                client_damage.x1(),
                client_damage.y1(),
                client_damage.x2(),
                client_damage.y2(),
                client_damage.width(),
                client_damage.height()
            ));
            run.errors.push("Actual output damage:".to_string());
            for (i, &actual_damage) in output_damage.iter().enumerate() {
                run.errors.push(format!(
                    "  [{}]: x1={}, y1={}, x2={}, y2={} ({}x{})",
                    i,
                    actual_damage.x1(),
                    actual_damage.y1(),
                    actual_damage.x2(),
                    actual_damage.y2(),
                    actual_damage.width(),
                    actual_damage.height()
                ));
            }
        }

        tassert!(found_exact_damage);
    }

    // Test 5b: Verify multiple surface damage rectangles create correct output damage
    connector_data.damage.borrow_mut().clear();

    // Add multiple damage rectangles
    let damage1 = Rect::new_sized(5, 5, 10, 10).unwrap();
    let damage2 = Rect::new_sized(20, 25, 15, 8).unwrap();

    window.surface.damage(
        damage1.x1(),
        damage1.y1(),
        damage1.width(),
        damage1.height(),
    )?;
    window.surface.damage(
        damage2.x1(),
        damage2.y1(),
        damage2.width(),
        damage2.height(),
    )?;
    window.surface.commit()?;
    client.sync().await;

    // Verify both damage rectangles are transformed correctly
    {
        let output_damage = connector_data.damage.borrow();
        tassert!(!output_damage.is_empty());

        let expected_damage1 = damage1.move_(surface_pos.x1(), surface_pos.y1());
        let expected_damage2 = damage2.move_(surface_pos.x1(), surface_pos.y1());

        let mut found_damage1 = false;
        let mut found_damage2 = false;

        for &actual_damage in output_damage.iter() {
            if actual_damage.x1() == expected_damage1.x1()
                && actual_damage.y1() == expected_damage1.y1()
                && actual_damage.x2() == expected_damage1.x2()
                && actual_damage.y2() == expected_damage1.y2()
            {
                found_damage1 = true;
            }
            if actual_damage.x1() == expected_damage2.x1()
                && actual_damage.y1() == expected_damage2.y1()
                && actual_damage.x2() == expected_damage2.x2()
                && actual_damage.y2() == expected_damage2.y2()
            {
                found_damage2 = true;
            }
        }

        if !found_damage1 || !found_damage2 {
            run.errors.push(format!("Multiple damage test failed:"));
            run.errors.push(format!(
                "Expected damage1: x1={}, y1={}, x2={}, y2={}",
                expected_damage1.x1(),
                expected_damage1.y1(),
                expected_damage1.x2(),
                expected_damage1.y2()
            ));
            run.errors.push(format!(
                "Expected damage2: x1={}, y1={}, x2={}, y2={}",
                expected_damage2.x1(),
                expected_damage2.y1(),
                expected_damage2.x2(),
                expected_damage2.y2()
            ));
            run.errors.push(format!(
                "Found damage1: {}, Found damage2: {}",
                found_damage1, found_damage2
            ));
        }

        tassert!(found_damage1);
        tassert!(found_damage2);
    }

    // Test 6: Verify buffer damage creates correct output damage with exact coordinates
    connector_data.damage.borrow_mut().clear();

    // Add buffer damage within the buffer bounds (spbm creates 1x1 pixel buffers)
    // Buffer damage must be within buffer.buffer.rect or it gets clipped to empty
    let buffer_damage = Rect::new_sized(0, 0, 1, 1).unwrap(); // Entire 1x1 buffer
    window.surface.damage_buffer(
        buffer_damage.x1(),
        buffer_damage.y1(),
        buffer_damage.width(),
        buffer_damage.height(),
    )?;
    window.surface.commit()?;
    client.sync().await;

    // Verify buffer damage was transformed correctly to output coordinates
    {
        let output_damage = connector_data.damage.borrow();
        tassert!(!output_damage.is_empty());

        // Buffer damage is transformed by the damage matrix which includes the surface position
        // The buffer damage (0,0,1,1) should be transformed to surface coordinates
        let expected_buffer_damage = buffer_damage.move_(surface_pos.x1(), surface_pos.y1());

        // Find the exact output damage that matches our expected buffer damage
        let mut found_exact_buffer_damage = false;
        for &actual_damage in output_damage.iter() {
            if actual_damage.x1() == expected_buffer_damage.x1()
                && actual_damage.y1() == expected_buffer_damage.y1()
                && actual_damage.x2() == expected_buffer_damage.x2()
                && actual_damage.y2() == expected_buffer_damage.y2()
            {
                found_exact_buffer_damage = true;
                break;
            }
        }

        tassert!(found_exact_buffer_damage);
    }

    // Test 7: Check output damage from existing window's viewport (which already has scaling)
    connector_data.damage.borrow_mut().clear();

    // The existing window was created with create_surface_ext() which automatically creates a viewport
    // Let's verify that the viewport's existing scaling affects buffer damage correctly
    // First, let's modify the viewport scaling that already exists on the window
    window.surface.viewport.set_destination(150, 100)?; // Change scaling to 150x100

    // Add buffer damage to test viewport scaling coordinate transformation
    window.surface.damage_buffer(0, 0, 1, 1)?; // Damage entire 1x1 buffer
    window.surface.commit()?;
    client.sync().await;

    // Verify the created output damage from viewporter coordinate transformation
    {
        let output_damage = connector_data.damage.borrow();
        tassert!(!output_damage.is_empty());

        // With viewporter scaling, the 1x1 buffer damage should scale to 150x100
        // and be moved by surface position (0, 36) to get output coordinates (0, 36, 150, 136)
        let expected_scaled_damage = Rect::new_sized(0, 0, 150, 100).unwrap();
        let expected_output_damage =
            expected_scaled_damage.move_(surface_pos.x1(), surface_pos.y1());

        // Find the exact scaled buffer damage in the output damage
        let mut found_viewport_scaled_damage = false;
        for &actual_damage in output_damage.iter() {
            if actual_damage.x1() == expected_output_damage.x1()
                && actual_damage.y1() == expected_output_damage.y1()
                && actual_damage.x2() == expected_output_damage.x2()
                && actual_damage.y2() == expected_output_damage.y2()
            {
                found_viewport_scaled_damage = true;
                break;
            }
        }

        if !found_viewport_scaled_damage {
            run.errors
                .push("Viewport-scaled buffer damage verification failed:".to_string());
            run.errors.push(format!(
                "Expected output damage: x1={}, y1={}, x2={}, y2={} ({}x{})",
                expected_output_damage.x1(),
                expected_output_damage.y1(),
                expected_output_damage.x2(),
                expected_output_damage.y2(),
                expected_output_damage.width(),
                expected_output_damage.height()
            ));
            run.errors.push("Actual output damage:".to_string());
            for (i, &actual_damage) in output_damage.iter().enumerate() {
                run.errors.push(format!(
                    "  [{}]: x1={}, y1={}, x2={}, y2={} ({}x{})",
                    i,
                    actual_damage.x1(),
                    actual_damage.y1(),
                    actual_damage.x2(),
                    actual_damage.y2(),
                    actual_damage.width(),
                    actual_damage.height()
                ));
            }
        }

        tassert!(found_viewport_scaled_damage);
    }

    // Test 8: Verify buffer transform rotation integrates with damage coordinate transformation
    // Create a surface with buffer transform rotation to test coordinate transformation
    let rotation_window = client.create_window().await?;

    rotation_window.map().await?;
    client.sync().await;

    // Disable viewporter by setting destination to 0x0 to rely purely on buffer dimensions
    rotation_window.surface.viewport.set_destination(0, 0)?; // Disable viewporter

    // Use a rectangular buffer (4x2) so rotation has a visible geometric effect
    // Attach AFTER mapping to avoid being overwritten by map()'s single-pixel buffer
    let rotation_buffer = client.shm.create_buffer(4, 2)?;
    rotation_window.surface.attach(rotation_buffer.buffer.id)?;
    rotation_window.surface.set_buffer_transform(1)?; // TF_90 = 1 (90 degrees rotation)
    rotation_window.surface.commit()?; // Commit the new buffer and transform
    client.sync().await;

    // Get the rotated surface position for damage coordinate verification
    let rotation_surface_pos = rotation_window.surface.server.buffer_abs_pos.get();

    // Clear damage immediately before the commit we want to test
    connector_data.damage.borrow_mut().clear();

    // Test buffer damage on rotated surface - damage entire buffer
    rotation_window.surface.damage_buffer(0, 0, 4, 2)?; // Damage entire 4x2 buffer
    rotation_window.surface.commit()?;
    client.sync().await;

    // Verify buffer damage creates exact output damage coordinates with buffer transform applied
    {
        let output_damage = connector_data.damage.borrow();
        tassert!(!output_damage.is_empty());

        // With buffer transform (90° rotation) and no viewporter:
        // Original 4x2 buffer becomes 2x4 after 90° rotation
        // Full buffer damage should result in full surface damage (2x4)
        // This verifies that rotation transforms the buffer dimensions correctly
        let expected_rotated_damage = Rect::new_sized(0, 0, 2, 4).unwrap(); // 4x2 buffer rotated to 2x4
        let expected_output_damage =
            expected_rotated_damage.move_(rotation_surface_pos.x1(), rotation_surface_pos.y1());

        // Find the exact transformed buffer damage in the output damage
        let mut found_exact_rotation_damage = false;
        for &actual_damage in output_damage.iter() {
            if actual_damage.x1() == expected_output_damage.x1()
                && actual_damage.y1() == expected_output_damage.y1()
                && actual_damage.x2() == expected_output_damage.x2()
                && actual_damage.y2() == expected_output_damage.y2()
            {
                found_exact_rotation_damage = true;
                break;
            }
        }

        if !found_exact_rotation_damage {
            run.errors
                .push("Buffer transform rotation exact coordinate test failed:".to_string());
            run.errors.push(format!(
                "Expected exact output damage: x1={}, y1={}, x2={}, y2={} ({}x{})",
                expected_output_damage.x1(),
                expected_output_damage.y1(),
                expected_output_damage.x2(),
                expected_output_damage.y2(),
                expected_output_damage.width(),
                expected_output_damage.height()
            ));
            run.errors.push(format!(
                "Rotated surface position: x1={}, y1={}",
                rotation_surface_pos.x1(),
                rotation_surface_pos.y1()
            ));
            run.errors
                .push("Applied: 4x2 buffer + 90-degree rotation (no viewporter)".to_string());
            run.errors
                .push("Actual output damage from buffer transform rotation:".to_string());
            for (i, &actual_damage) in output_damage.iter().enumerate() {
                run.errors.push(format!(
                    "  [{}]: x1={}, y1={}, x2={}, y2={} ({}x{})",
                    i,
                    actual_damage.x1(),
                    actual_damage.y1(),
                    actual_damage.x2(),
                    actual_damage.y2(),
                    actual_damage.width(),
                    actual_damage.height()
                ));
            }
        }

        tassert!(found_exact_rotation_damage);
    }

    // Test 9: Empty damage rectangles (edge case)
    connector_data.damage.borrow_mut().clear();
    window.surface.damage(0, 0, 0, 0)?; // Empty rect
    window.surface.commit()?;
    client.sync().await;

    // Empty damage should not crash the compositor (main requirement)
    // Whether it creates output damage or not is implementation-defined

    Ok(())
}
