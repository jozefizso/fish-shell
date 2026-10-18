//! ANSI byte-stream output for batch highlighting, independent of terminal and reader state.
use crate::text_face::{ColorSupport, ResettableStyle, TextFace, UnderlineStyle};
use fish_color::Color;
use fish_widestring::{wcs2bytes_appending, wstr};
use std::io::Write as _;

pub(super) struct AnsiBuffer {
    contents: Vec<u8>,
    last: TextFace,
    support: ColorSupport,
}

impl AnsiBuffer {
    pub(super) fn new(support: ColorSupport) -> Self {
        Self {
            contents: Vec::new(),
            last: TextFace::terminal_default(),
            support,
        }
    }

    pub(super) fn set_text_face(&mut self, mut face: TextFace) {
        let quantize = |color: Color| {
            if color.is_rgb() && !self.support.term_24bit {
                Color::Named {
                    idx: if self.support.term_256color {
                        color.to_term256_index()
                    } else {
                        color.to_name_index()
                    },
                }
            } else {
                color
            }
        };
        face.fg = quantize(face.fg);
        face.bg = quantize(face.bg);
        face.underline_color = quantize(face.underline_color);
        if face == self.last {
            return;
        }
        if face == TextFace::terminal_default() {
            self.contents.extend_from_slice(b"\x1b[0m");
            self.last = face;
            return;
        }

        write_color(&mut self.contents, &mut self.last.fg, face.fg, 38);
        write_color(&mut self.contents, &mut self.last.bg, face.bg, 48);
        write_color(
            &mut self.contents,
            &mut self.last.underline_color,
            face.underline_color,
            58,
        );

        let style = face.style;
        let last = &mut self.last.style;
        // SGR 22 clears both bold and dim; reapply the one that is still requested.
        if (last.bold && !style.bold) || (last.dim && !style.dim) {
            self.contents.extend_from_slice(b"\x1b[22m");
            last.bold = false;
            last.dim = false;
        }
        if style.bold && !last.bold {
            self.contents.extend_from_slice(b"\x1b[1m");
            last.bold = true;
        }
        if style.dim && !last.dim {
            self.contents.extend_from_slice(b"\x1b[2m");
            last.dim = true;
        }
        if style.underline_style != last.underline_style {
            let sequence: &[u8] = match style.underline_style {
                ResettableStyle::Unchanged => b"",
                ResettableStyle::Off => b"\x1b[24m",
                ResettableStyle::On(UnderlineStyle::Single) => b"\x1b[4m",
                ResettableStyle::On(UnderlineStyle::Double) => b"\x1b[4:2m",
                ResettableStyle::On(UnderlineStyle::Curly) => b"\x1b[4:3m",
                ResettableStyle::On(UnderlineStyle::Dotted) => b"\x1b[4:4m",
                ResettableStyle::On(UnderlineStyle::Dashed) => b"\x1b[4:5m",
            };
            if !sequence.is_empty() {
                self.contents.extend_from_slice(sequence);
                last.underline_style = style.underline_style;
            }
        }
        write_style(
            &mut self.contents,
            &mut last.italics,
            style.italics,
            b"\x1b[3m",
            b"\x1b[23m",
        );
        write_style(
            &mut self.contents,
            &mut last.reverse,
            style.reverse,
            b"\x1b[7m",
            b"\x1b[27m",
        );
        write_style(
            &mut self.contents,
            &mut last.strikethrough,
            style.strikethrough,
            b"\x1b[9m",
            b"\x1b[29m",
        );
    }

    pub(super) fn writech(&mut self, ch: char) {
        wcs2bytes_appending(&mut self.contents, wstr::from_char_slice(&[ch]));
    }

    pub(super) fn take_contents(self) -> Vec<u8> {
        self.contents
    }
}

fn write_color(out: &mut Vec<u8>, last: &mut Color, color: Color, target: u8) {
    if color.is_none() || color == *last {
        return;
    }
    match color {
        Color::Normal => write!(out, "\x1b[{}m", target + 1).unwrap(),
        Color::Named { idx } if idx < 16 && target != 58 => {
            let code = target - 8 + idx % 8 + if idx >= 8 { 60 } else { 0 };
            write!(out, "\x1b[{code}m").unwrap();
        }
        Color::Named { idx } => write!(out, "\x1b[{target};5;{idx}m").unwrap(),
        Color::Rgb(rgb) => {
            write!(out, "\x1b[{target};2;{};{};{}m", rgb.r, rgb.g, rgb.b).unwrap();
        }
        Color::None => unreachable!(),
    }
    *last = color;
}

fn write_style(
    out: &mut Vec<u8>,
    last: &mut ResettableStyle,
    style: ResettableStyle,
    enter: &[u8],
    exit: &[u8],
) {
    if style == *last {
        return;
    }
    let sequence = match style {
        ResettableStyle::Unchanged => return,
        ResettableStyle::Off => exit,
        ResettableStyle::On(()) => enter,
    };
    out.extend_from_slice(sequence);
    *last = style;
}

#[cfg(test)]
mod tests {
    use super::*;
    const FULL_COLOR: ColorSupport = ColorSupport {
        term_256color: true,
        term_24bit: true,
    };

    #[test]
    fn ansi_preserves_wide_text_and_escaped_bytes() {
        let mut output = AnsiBuffer::new(FULL_COLOR);
        output.writech('🦀');
        output.writech(fish_widestring::encode_byte_to_char(0xff));
        output.writech('\n');
        output.set_text_face(TextFace::terminal_default());
        assert_eq!(output.take_contents(), b"\xf0\x9f\xa6\x80\xff\n");
    }
}
