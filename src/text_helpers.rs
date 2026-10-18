use crate::prelude::*;
use fish_fallback::fish_wcwidth;
use fish_feature_flags::{FeatureFlag, feature_test};
use fish_widestring::decode_byte_from_char;

/// Return the character length of a recognized terminal escape sequence.
pub fn escape_code_length(code: &wstr) -> Option<usize> {
    if code.char_at(0) != '\x1B' {
        return None;
    }
    is_screen_name_escape_seq(code)
        .or_else(|| is_osc_escape_seq(code))
        .or_else(|| is_three_byte_escape_seq(code))
        .or_else(|| is_csi_style_escape_seq(code))
        .or_else(|| is_two_byte_escape_seq(code))
}

fn is_screen_name_escape_seq(code: &wstr) -> Option<usize> {
    let tmux_seq = L!("Ptmux;");
    let mut is_tmux = false;
    if code.char_at(1) != 'k' {
        if code.starts_with(tmux_seq) {
            is_tmux = true;
        } else {
            return None;
        }
    }
    let sentinel = L!("\x1B\\");
    let mut offset = 2;
    loop {
        let Some(pos) = code[offset..].find(sentinel) else {
            return Some(2);
        };
        let end = offset + pos;
        if is_tmux {
            let mut count = 0;
            let mut i = end;
            while i > 0 && code.as_char_slice()[i - 1] == '\x1B' {
                i -= 1;
                if i > 0 {
                    count += 1;
                }
            }
            if count % 2 == 1 {
                offset = end + 1;
                continue;
            }
        }
        return Some(end + sentinel.len());
    }
}

fn is_osc_escape_seq(code: &wstr) -> Option<usize> {
    if code.char_at(1) == ']' {
        let mut cursor = 2;
        while cursor < code.len() {
            let code = code.as_char_slice();
            if code[cursor] == '\x07' || (code[cursor] == '\\' && code[cursor - 1] == '\x1B') {
                return Some(cursor + 1);
            }
            cursor += 1;
        }
    }
    None
}

fn is_three_byte_escape_seq(code: &wstr) -> Option<usize> {
    if code.char_at(1) == '[' && (code.char_at(2) >= '@' && code.char_at(2) <= '_') {
        return Some(3);
    }
    None
}

fn is_two_byte_escape_seq(code: &wstr) -> Option<usize> {
    if code.char_at(1) >= '@' && code.char_at(1) <= '_' {
        return Some(2);
    }
    None
}

fn is_csi_style_escape_seq(code: &wstr) -> Option<usize> {
    if code.char_at(1) != '[' {
        return None;
    }
    let mut cursor = 2;
    while cursor < code.len() {
        let c = code.as_char_slice()[cursor];
        if !c.is_ascii() {
            break;
        }
        if ('@'..'~').contains(&c) {
            cursor += 1;
            break;
        }
        cursor += 1;
    }
    Some(cursor)
}

fn ctrl_to_symbol(buf: &mut WString, c: char) {
    let c = u8::try_from(c).unwrap();
    let name = match c {
        9 => L!("\\t"),
        13 => L!("\\r"),
        27 => L!("\\e"),
        _ => return sprintf!(=> buf, "\\x%02x", c),
    };
    buf.push_utfstr(name);
}

fn must_escape(is_first_in_token: bool, c: char) -> bool {
    "[]()<>{}*\\$;&|'\"".contains(c)
        || (is_first_in_token && "~#".contains(c))
        || (c == '?' && !feature_test(FeatureFlag::QuestionMarkNoGlob))
}

fn ascii_printable_to_symbol(buf: &mut WString, is_first_in_token: bool, c: char) {
    if must_escape(is_first_in_token, c) {
        sprintf!(=> buf, "\\%c", c);
    } else {
        sprintf!(=> buf, "%c", c);
    }
}

/// Convert a character to a printable fish symbol, including zero-width input.
pub fn char_to_symbol(c: char, is_first_in_token: bool) -> WString {
    let mut buff = WString::new();
    let buf = &mut buff;
    if c <= ' ' || c == '\x7F' {
        ctrl_to_symbol(buf, c);
    } else if c < '\u{80}' {
        ascii_printable_to_symbol(buf, is_first_in_token, c);
    } else if let Some(byte) = decode_byte_from_char(c) {
        sprintf!(=> buf, "\\x%02x", byte);
    } else if ('\u{e000}'..='\u{f8ff}').contains(&c) {
        sprintf!(=> buf, "\\u%04X", u32::from(c));
    } else if fish_wcwidth(c).is_some_and(|w| w != 0) {
        sprintf!(=> buf, "%c", c);
    } else if c <= '\u{FFFF}' {
        sprintf!(=> buf, "\\u%04X", u32::from(c));
    } else {
        sprintf!(=> buf, "\\U%06X", u32::from(c));
    }
    buff
}
