use {
    crate::toml::toml_span::Span,
    bstr::ByteSlice,
    error_reporter::Report,
    std::{
        borrow::Cow,
        error::Error,
        fmt::{Display, Formatter},
        ops::Deref,
    },
};

#[derive(Debug)]
pub struct SpannedError<'a, E> {
    pub input: Cow<'a, [u8]>,
    pub span: Span,
    pub cause: Option<E>,
}

impl<E: Error> Display for SpannedError<'_, E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let original = self.input.deref();
        let span = self.span;

        let (line, column) = translate_position(original, span.lo);
        let line_num = line + 1;
        let col_num = column + 1;
        let gutter = line_num.to_string().len();
        let content = original
            .split(|c| *c == b'\n')
            .nth(line)
            .expect("valid line number");

        if let Some(cause) = &self.cause {
            write!(f, "{}: ", Report::new(cause))?;
        }
        writeln!(f, "At line {}, column {}:", line_num, col_num)?;
        for _ in 0..=gutter {
            write!(f, " ")?;
        }
        writeln!(f, "|")?;
        write!(f, "{} | ", line_num)?;
        writeln!(f, "{}", content.as_bstr())?;
        for _ in 0..=gutter {
            write!(f, " ")?;
        }
        write!(f, "|")?;
        for _ in 0..=column {
            write!(f, " ")?;
        }
        write!(f, "^")?;
        for _ in (span.lo + 1)..(span.hi.min(span.lo + content.len() - column)) {
            write!(f, "^")?;
        }

        Ok(())
    }
}

impl<E: Error> Error for SpannedError<'_, E> {}

fn translate_position(input: &[u8], index: usize) -> (usize, usize) {
    if input.is_empty() {
        return (0, index);
    }

    let safe_index = index.min(input.len() - 1);
    let column_offset = index - safe_index;
    let index = safe_index;

    let nl = input[0..index]
        .iter()
        .rev()
        .enumerate()
        .find(|(_, b)| **b == b'\n')
        .map(|(nl, _)| index - nl - 1);
    let line_start = match nl {
        Some(nl) => nl + 1,
        None => 0,
    };
    let line = input[0..line_start].iter().filter(|b| **b == b'\n').count();

    let column = std::str::from_utf8(&input[line_start..=index])
        .map(|s| s.chars().count() - 1)
        .unwrap_or_else(|_| index - line_start);
    let column = column + column_offset;

    (line, column)
}
