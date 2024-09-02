use {
    crate::it::{test_error::TestResult, testrun::TestRun},
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    run.sync().await;

    tassert!(!run.cfg.graphics_initialized.get());

    run.backend.install_render_context(false)?;

    tassert!(run.cfg.graphics_initialized.get());

    Ok(())
}
