//! Knobs for changing the status text.

use {
    crate::{exec::Command, io::Async, tasks::spawn},
    bstr::ByteSlice,
    error_reporter::Report,
    futures_util::{AsyncBufReadExt, io::BufReader},
    serde::Deserialize,
    std::borrow::BorrowMut,
    uapi::{OwnedFd, c},
};

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

/// The format of a status command output.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum MessageFormat {
    /// The output is plain text.
    ///
    /// The command should output one line every time it wants to change the status.
    /// The content of the line will be interpreted as plain text.
    Plain,
    /// The output uses [pango][pango] markup.
    ///
    /// The command should output one line every time it wants to change the status.
    /// The content of the line will be interpreted as pango markup.
    ///
    /// [pango]: https://docs.gtk.org/Pango/pango_markup.html
    Pango,
    /// The output uses the [i3bar][i3bar] protocol.
    ///
    /// The separator between individual components can be set using [`set_i3bar_separator`].
    ///
    /// [i3bar]: https://github.com/i3/i3/blob/next/docs/i3bar-protocol
    I3Bar,
}

/// Sets a command whose output will be used as the status text.
///
/// The [`stdout`](Command::stdout) and [`stderr`](Command::stderr)` of the command will
/// be overwritten by this function. The stdout will be used for the status text and the
/// stderr will be appended to the compositor log.
///
/// The format of stdout is determined by the `format` parameter.
pub fn set_status_command(format: MessageFormat, mut command: impl BorrowMut<Command>) {
    macro_rules! pipe {
        () => {{
            let (read, write) = match uapi::pipe2(c::O_CLOEXEC) {
                Ok(p) => p,
                Err(e) => {
                    log::error!("Could not create a pipe: {}", Report::new(e));
                    return;
                }
            };
            let read = match Async::new(read) {
                Ok(r) => BufReader::new(r),
                Err(e) => {
                    log::error!("Could not create an Async object: {}", Report::new(e));
                    return;
                }
            };
            (read, write)
        }};
    }
    let (mut read, write) = pipe!();
    let (mut stderr_read, stderr_write) = pipe!();
    let command = command.borrow_mut();
    command.stdout(write).stderr(stderr_write).spawn();
    let name = command.prog.clone();
    let name2 = command.prog.clone();
    let stderr_handle = spawn(async move {
        let mut line = vec![];
        loop {
            line.clear();
            if let Err(e) = stderr_read.read_until(b'\n', &mut line).await {
                log::warn!("Could not read from {name2} stderr: {}", Report::new(e));
                return;
            }
            if line.len() == 0 {
                return;
            }
            log::warn!(
                "{name2} emitted a message on stderr: {}",
                line.trim_with(|c| c == '\n').as_bstr()
            );
        }
    });
    let handle = spawn(async move {
        if format == MessageFormat::I3Bar {
            handle_i3bar(name, read).await;
            return;
        }
        let mut line = String::new();
        let mut cleaned = String::new();
        loop {
            line.clear();
            if let Err(e) = read.read_line(&mut line).await {
                log::error!("Could not read from `{name}`: {}", Report::new(e));
                return;
            }
            if line.is_empty() {
                log::info!("{name} closed stdout");
                return;
            }
            let line = line.strip_suffix("\n").unwrap_or(&line);
            cleaned.clear();
            if format != MessageFormat::Pango && escape_pango(line, &mut cleaned) {
                set_status(&cleaned);
            } else {
                set_status(line);
            }
        }
    });
    get!().set_status_tasks(vec![handle, stderr_handle]);
}

/// Unsets the previously set status command.
pub fn unset_status_command() {
    get!().set_status_tasks(vec![]);
}

/// Sets the separator for i3bar status commands.
///
/// The separator should be specified in [pango][pango] markup language.
///
/// [pango]: https://docs.gtk.org/Pango/pango_markup.html
pub fn set_i3bar_separator(separator: &str) {
    get!().set_i3bar_separator(separator);
}

async fn handle_i3bar(name: String, mut read: BufReader<Async<OwnedFd>>) {
    use std::fmt::Write;

    #[derive(Deserialize)]
    struct Version {
        version: i32,
    }
    #[derive(Deserialize)]
    struct Component {
        markup: Option<String>,
        full_text: String,
        color: Option<String>,
        background: Option<String>,
    }
    let mut line = String::new();
    macro_rules! read_line {
        () => {{
            line.clear();
            if let Err(e) = read.read_line(&mut line).await {
                log::error!("Could not read from `{name}`: {}", Report::new(e));
                return;
            }
            if line.is_empty() {
                log::info!("{name} closed stdout");
                return;
            }
        }};
    }
    read_line!();
    match serde_json::from_str::<Version>(&line) {
        Ok(v) if v.version == 1 => {}
        Ok(v) => log::warn!("Unexpected i3bar format version: {}", v.version),
        Err(e) => {
            log::warn!(
                "Could not deserialize i3bar version message: {}",
                Report::new(e)
            );
            return;
        }
    }
    read_line!();
    let mut status = String::new();
    loop {
        read_line!();
        let mut line = line.as_str();
        if let Some(l) = line.strip_prefix(",") {
            line = l;
        }
        let components = match serde_json::from_str::<Vec<Component>>(line) {
            Ok(c) => c,
            Err(e) => {
                log::warn!(
                    "Could not deserialize i3bar status message: {}",
                    Report::new(e)
                );
                continue;
            }
        };
        let separator = get!().get_i3bar_separator();
        let separator = match &separator {
            Some(s) => s.as_str(),
            _ => r##" <span color="#333333">|</span> "##,
        };
        status.clear();
        let mut first = true;
        for component in &components {
            if component.full_text.is_empty() {
                continue;
            }
            if !first {
                status.push_str(separator);
            }
            first = false;
            let have_span = component.color.is_some() || component.background.is_some();
            if have_span {
                status.push_str("<span");
                if let Some(color) = &component.color {
                    let _ = write!(status, r#" color="{color}""#);
                }
                if let Some(color) = &component.background {
                    let _ = write!(status, r#" bgcolor="{color}""#);
                }
                status.push_str(">");
            }
            if component.markup.as_deref() == Some("pango")
                || !escape_pango(&component.full_text, &mut status)
            {
                status.push_str(&component.full_text);
            }
            if have_span {
                status.push_str("</span>");
            }
        }
        set_status(&status);
    }
}

fn escape_pango(src: &str, dst: &mut String) -> bool {
    if src
        .bytes()
        .any(|b| matches!(b, b'&' | b'<' | b'>' | b'\'' | b'"'))
    {
        for c in src.chars() {
            match c {
                '&' => dst.push_str("&amp;"),
                '<' => dst.push_str("&lt;"),
                '>' => dst.push_str("&gt;"),
                '\'' => dst.push_str("&apos;"),
                '"' => dst.push_str("&quot;"),
                _ => dst.push(c),
            }
        }
        true
    } else {
        false
    }
}
