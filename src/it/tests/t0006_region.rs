use {
    crate::{
        it::{test_error::TestError, testrun::TestRun},
        rect::Rect,
    },
    std::rc::Rc,
};

testcase!();

/// Test region creation
async fn test(run: Rc<TestRun>) -> Result<(), TestError> {
    let client = run.create_client().await?;

    let region = client.comp.create_region().await?;
    region.check().await?;
    region.add(Rect::new(10, 20, 30, 40).unwrap())?;
    region.check().await?;
    region.subtract(Rect::new(15, 25, 25, 35).unwrap())?;
    region.check().await?;

    let expected = region.expected.borrow_mut().get();

    tassert_eq!(expected.extents(), Rect::new(10, 20, 30, 40).unwrap());
    tassert_eq!(expected.len(), 4);
    tassert_eq!(expected[0], Rect::new(10, 20, 30, 25).unwrap());
    tassert_eq!(expected[1], Rect::new(10, 25, 15, 35).unwrap());
    tassert_eq!(expected[2], Rect::new(25, 25, 30, 35).unwrap());
    tassert_eq!(expected[3], Rect::new(10, 35, 30, 40).unwrap());

    Ok(())
}
