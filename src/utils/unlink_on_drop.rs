pub struct UnlinkOnDrop<'a>(pub &'a str);

impl<'a> Drop for UnlinkOnDrop<'a> {
    fn drop(&mut self) {
        let _ = uapi::unlink(self.0);
    }
}
