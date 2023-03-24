use std::fmt;

pub(crate) fn human_size(bytes: impl humansize::ToF64 + humansize::Unsigned) -> String {
    humansize::format_size(bytes, humansize::BINARY)
}

pub(crate) fn bold(val: &dyn fmt::Display) -> impl fmt::Display {
    nu_ansi_term::Style::new().bold().paint(val.to_string())
}

pub(crate) fn bold_human_size(bytes: usize) -> impl fmt::Display {
    bold(&human_size(bytes))
}

pub(crate) fn elpased(start: std::time::Instant) -> impl fmt::Display {
    nu_ansi_term::Color::Blue
        .bold()
        .paint(format!("{:.2?}", start.elapsed()))
}
