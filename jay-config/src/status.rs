//! Knobs for changing the status text.

/// Sets the status text.
///
/// The status text is displayed at the right end of the bar.
///
/// The status text should be specified in [pango][pango] markup language.
///
/// [pango]: https://docs.gtk.org/Pango/pango_markup.html
pub fn set_status(status: &str) {
    get!().set_status(status);
}
