use crate::ifs::wp_content_type_v1::ContentType;
use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::testrun::TestRun;
use std::rc::Rc;

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let _ds = run.create_default_setup().await?;

    let client = run.create_client()?;
    let surface = client.create_surface().await?;
    let ct = client.get_surface_content_type(&surface);
    ct.set_content_type(2);
    surface.commit();
    client.sync().await;

    tassert_eq!(surface.server.content_type.get(), Some(ContentType::Video));

    Ok(())
}
