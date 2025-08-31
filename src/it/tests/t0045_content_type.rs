use {
    crate::{
        ifs::wp_content_type_v1::ContentType,
        it::{test_error::TestResult, testrun::TestRun},
    },
    std::rc::Rc,
};

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let _ds = run.create_default_setup().await?;

    let client = run.create_client().await?;
    let surface = client.comp.create_surface().await?;
    let ctm = client.registry.get_content_type_manager().await?;
    let ct = ctm.get_surface_content_type(&surface)?;
    ct.set_content_type(2)?;
    surface.commit()?;
    client.sync().await;

    tassert_eq!(surface.server.content_type.get(), Some(ContentType::Video));

    Ok(())
}
