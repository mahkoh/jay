use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::testrun::TestRun;
use std::rc::Rc;

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let client = run.create_client()?;
    let registry1 = client.new_registry();

    client.sync().await;
    let before = client.client.objects.dedicated.wl_registry.len();

    registry1.destroy();

    client.sync().await;
    let after = client.client.objects.dedicated.wl_registry.len();

    tassert_eq!(before, after + 1);

    let registry2 = client.new_registry();
    client.sync().await;

    tassert!(registry2.globals.is_not_empty());

    Ok(())
}
