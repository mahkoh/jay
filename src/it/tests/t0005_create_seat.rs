use {
    crate::{
        globals::GlobalBase,
        it::{test_error::TestError, testrun::TestRun},
    },
    std::rc::Rc,
};

testcase!();

/// Test seat creation and broadcast
async fn test(run: Rc<TestRun>) -> Result<(), TestError> {
    let client = run.create_client().await?;

    tassert!(client.registry.seats.is_empty());

    let seat = run.get_seat("default")?;

    client.sync().await;

    tassert_eq!(client.registry.seats.len(), 1);

    let client_seat = client.registry.seats.get(&seat.name());
    tassert!(client_seat.is_some());

    let client_seat = client_seat.unwrap();

    tassert_eq!(seat.id(), client_seat.id());

    Ok(())
}
