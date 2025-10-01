use {
    crate::{
        format::{ARGB8888, XRGB8888},
        it::{test_error::TestError, testrun::TestRun},
    },
    std::rc::Rc,
};

testcase!();

/// Test that wl_shm supports the required formats
async fn test(run: Rc<TestRun>) -> Result<(), TestError> {
    run.backend.install_render_context(false)?;
    let client = run.create_client().await?;
    let formats = client.shm.formats().await;
    tassert!(formats.contains(&XRGB8888.wl_id.unwrap()));
    tassert!(formats.contains(&ARGB8888.wl_id.unwrap()));
    Ok(())
}
