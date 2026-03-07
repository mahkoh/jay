pub trait StaticText {
    fn text(&self) -> &'static str;
}
