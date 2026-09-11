use crate::it::test_client::TestClientExt;
use crate::it::test_error::TestResult;
use crate::it::testrun::TestRun;
use crate::theme::Color;
use std::rc::Rc;

testcase!();

async fn test(run: Rc<TestRun>) -> TestResult {
    let _ds = run.create_default_setup().await?;

    let client = run.create_client()?;
    let win = client.create_window().await?;
    win.set_color(255, 0, 0, 255);
    win.map2().await?;

    macro_rules! create_surface {
        ($buf:expr, $x:expr, $y:expr) => {{
            let ss = client.create_surface().await?;
            let vp = client.get_viewport(&ss);
            vp.set_destination(100, 100);
            ss.attach($buf.id);
            ss.commit();
            let alpha = client.get_alpha_modifier_surface(&ss);
            let sub = client.get_subsurface(ss.id, win.surface.id);
            sub.set_desync();
            sub.set_position($x, $y);
            win.surface.commit();
            (ss, alpha)
        }};
    }

    let buf1 = client.create_single_pixel_buffer(Color::from_srgb(0, 255, 0));
    let (ss1, alpha1) = create_surface!(&buf1, 0, 0);

    let buf2 = client.create_shm_buffer(1, 1)?;
    buf2.fill(Color::from_srgb(0, 255, 0));
    let (ss2, alpha2) = create_surface!(&buf2.buffer, 100, 0);

    client.compare_screenshot("1", false).await?;

    alpha1.set_multiplier(0.51);
    ss1.commit();
    alpha2.set_multiplier(0.51);
    ss2.commit();

    client.compare_screenshot("2", false).await?;

    Ok(())
}
