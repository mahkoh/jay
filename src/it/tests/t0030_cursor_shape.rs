use {
    crate::{
        cursor::KnownCursor,
        it::{test_error::TestResult, testrun::TestRun},
    },
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let ds = run.create_default_setup().await?;

    let client = run.create_client().await?;

    let seat = client.get_default_seat().await?;
    let dev = client.cursor_shape_manager.get_pointer(&seat.pointer)?;
    let enter = seat.pointer.enter.expect()?;

    let win1 = client.create_window().await?;
    win1.map2().await?;

    dev.set_shape(enter.last()?.serial, 2)?;
    client.sync().await;

    tassert_eq!(
        ds.seat.pointer_cursor().desired_known_cursor(),
        Some(KnownCursor::ContextMenu)
    );

    Ok(())
}
