use {
    crate::{
        fixed::Fixed,
        it::{test_error::TestResult, testrun::TestRun},
        tree::Node,
    },
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    let client = run.create_client().await?;

    let seat = client.get_default_seat().await?;
    let enter = seat.pointer.enter.expect()?;

    let win1 = client.create_window().await?;
    win1.map2().await?;

    let (x, y) = win1.surface.server.node_mapped_position().position();
    ds.move_to(x, y);
    run.state.idle().await;
    client.sync().await;

    // Get the pointer warp manager through the client infrastructure
    let warp_manager = &client.pointer_warp;

    // Get the enter serial
    let enter_serial = enter.last()?.serial;

    // Test the pointer warp protocol by attempting to warp the pointer
    // The main goal is to verify that the protocol is properly implemented
    // and doesn't crash when used with valid parameters
    let warp_x = Fixed::from_int(200);
    let warp_y = Fixed::from_int(150);
    warp_manager.warp_pointer(&win1.surface, &seat.pointer, warp_x, warp_y, enter_serial)?;

    // Sync to ensure the warp request is processed without errors
    client.sync().await;
    run.state.idle().await;

    // Verify the exact cursor position after the warp
    let (cursor_x, cursor_y) = ds.seat.pointer_cursor().position();
    let expected_x = Fixed::from_int(x) + warp_x;
    let expected_y = Fixed::from_int(y) + warp_y;

    tassert_eq!(cursor_x, expected_x);
    tassert_eq!(cursor_y, expected_y);

    Ok(())
}
