use {
    crate::it::{test_error::TestError, testrun::TestRun},
    std::{future::pending, rc::Rc},
};

testcase!();

/// Quit
async fn test(run: Rc<TestRun>) -> Result<(), TestError> {
    for _ in 0..2 {
        run.state.eng.yield_now().await;
    }
    run.cfg.quit()?;
    pending().await
}
