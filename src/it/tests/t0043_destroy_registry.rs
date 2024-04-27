use {
    crate::it::{test_error::TestResult, testrun::TestRun},
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let client = run.create_client().await?;
    let wl_fixes = client.registry.get_wl_fixes().await?;

    let registry1 = client.tran.get_registry();

    client.sync().await;
    let before = client.server.objects.registries.len();

    wl_fixes.destroy_registry(&registry1)?;

    client.sync().await;
    let after = client.server.objects.registries.len();

    tassert_eq!(before, after + 1);

    let registry2 = client.tran.get_registry();
    client.sync().await;

    tassert_eq!(registry1.id, registry2.id);
    tassert!(registry2.globals.is_not_empty());

    Ok(())
}
