#[expect(unused)]
pub struct LogOnDrop(&'static str);

impl Drop for LogOnDrop {
    fn drop(&mut self) {
        log::info!("{}", self.0);
    }
}
