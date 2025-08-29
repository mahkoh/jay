#[expect(dead_code)]
pub struct LogOnDrop(pub &'static str);

impl Drop for LogOnDrop {
    fn drop(&mut self) {
        log::info!("{}", self.0);
    }
}
