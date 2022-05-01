use {
    crate::{
        format::ARGB8888,
        it::{test_error::TestError, testrun::TestRun},
        theme::Color,
    },
    std::rc::Rc,
};

testcase!();

/// Create and map a single surface
async fn test(run: Rc<TestRun>) -> Result<(), TestError> {
    run.backend.install_default();

    let client = run.create_client().await?;
    let surface = client.comp.create_surface().await?;
    let xdg_surface = client.xdg.create_xdg_surface(surface.id).await?;
    let xdg_toplevel = xdg_surface.create_toplevel().await?;
    surface.commit();
    client.sync().await;

    {
        let pool = client.shm.create_pool(0)?;
        let buffer = pool.create_buffer(0, 0, 0, 0, ARGB8888)?;
        xdg_surface.ack_configure(xdg_surface.last_serial.get());
        surface.attach(buffer.id);
        surface.commit();
        client.sync().await;
    }

    tassert_eq!(xdg_toplevel.width.get(), 800);
    tassert!(xdg_toplevel.height.get() >= 500);

    {
        let pool = client
            .shm
            .create_pool((xdg_toplevel.width.get() * xdg_toplevel.height.get() * 4) as _)?;
        let buffer = pool.create_buffer(
            0,
            xdg_toplevel.width.get(),
            xdg_toplevel.height.get(),
            xdg_toplevel.width.get() * 4,
            ARGB8888,
        )?;
        buffer.fill(Color::from_rgba_straight(255, 0, 0, 100));
        xdg_surface.ack_configure(xdg_surface.last_serial.get());
        surface.attach(buffer.id);
        surface.commit();
    }

    let screenshot = client.take_screenshot().await?;
    std::fs::write(format!("{}/screenshot.qoi", run.dir), screenshot)?;

    Ok(())
}
