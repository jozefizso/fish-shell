//! Scalar boundaries in canonical shell text, without changing its parser representation.
//!
//! Literal reserved Unicode is stored as UTF-8 bytes in fish's byte-marker characters. A complete,
//! valid encoded scalar is one editing unit; malformed bytes remain independent editing units.
//! Ordinary Unicode, including combining characters, retains its existing codepoint boundaries.

use fish_widestring::decode_byte_from_char;

use crate::prelude::*;

/// Length of the scalar at `start`, or one for an ordinary character or malformed byte.
/// The caller must supply an index smaller than the string's length.
fn scalar_len(text: &wstr, start: usize) -> usize {
    let Some(first) = decode_byte_from_char(text.char_at(start)) else {
        return 1;
    };
    let len = match first {
        0xc2..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf4 => 4,
        _ => return 1,
    };
    if text.len() - start < len {
        return 1;
    }
    let mut bytes = [0; 4];
    bytes[0] = first;
    for (offset, byte) in bytes.iter_mut().enumerate().take(len).skip(1) {
        let Some(value) = decode_byte_from_char(text.char_at(start + offset)) else {
            return 1;
        };
        *byte = value;
    }
    // This also rejects overlong encodings, surrogates, and values above U+10FFFF.
    if std::str::from_utf8(&bytes[..len]).is_ok() {
        len
    } else {
        1
    }
}

/// The boundary at or before `pos`. All positions use canonical `wstr` indices.
/// Panics if `pos` exceeds the text length. Looks at at most three preceding characters.
pub fn floor_boundary(text: &wstr, pos: usize) -> usize {
    assert!(pos <= text.len());
    for start in pos.saturating_sub(3)..pos {
        if start + scalar_len(text, start) > pos {
            return start;
        }
    }
    pos
}

/// The boundary at or after `pos` (which must not exceed the text length).
pub fn ceil_boundary(text: &wstr, pos: usize) -> usize {
    let start = floor_boundary(text, pos);
    if start == pos {
        pos
    } else {
        start + scalar_len(text, start)
    }
}

/// The first boundary strictly after `pos`, or the text end if already there.
/// For an interior position this returns the end of its containing encoded scalar.
pub fn next_boundary(text: &wstr, pos: usize) -> usize {
    let start = floor_boundary(text, pos);
    if start == text.len() {
        start
    } else {
        start + scalar_len(text, start)
    }
}

/// The first boundary strictly before `pos`, or zero if already there.
/// For an interior position this returns the start of its containing encoded scalar.
pub fn previous_boundary(text: &wstr, pos: usize) -> usize {
    let start = floor_boundary(text, pos);
    if start < pos || pos == 0 {
        start
    } else {
        floor_boundary(text, pos - 1)
    }
}

/// Count editing scalars, retaining malformed byte markers as individual units.
pub fn count_scalars(text: &wstr) -> usize {
    let mut pos = 0;
    let mut count = 0;
    while pos < text.len() {
        pos += scalar_len(text, pos);
        count += 1;
    }
    count
}

/// The canonical end index of at most `count` complete editing scalars.
/// A count beyond the available scalars returns the text length.
pub fn prefix_end(text: &wstr, count: usize) -> usize {
    let mut pos = 0;
    for _ in 0..count {
        if pos == text.len() {
            break;
        }
        pos += scalar_len(text, pos);
    }
    pos
}

#[cfg(test)]
mod tests {
    use super::*;
    use fish_widestring::encode_byte_to_char;

    fn bytes(bytes: &[u8]) -> WString {
        bytes.iter().copied().map(encode_byte_to_char).collect()
    }

    #[test]
    fn encoded_scalar_boundaries_and_prefixes() {
        // U+F600 is reserved internally and therefore represented by three byte markers.
        let text = bytes(&[0xef, 0x98, 0x80]);
        for pos in 1..3 {
            assert_eq!(floor_boundary(&text, pos), 0);
            assert_eq!(ceil_boundary(&text, pos), 3);
            assert_eq!(next_boundary(&text, pos), 3);
            assert_eq!(previous_boundary(&text, pos), 0);
        }
        assert_eq!(next_boundary(&text, 0), 3);
        assert_eq!(next_boundary(&text, 3), 3);
        assert_eq!(previous_boundary(&text, 3), 0);
        assert_eq!(previous_boundary(&text, 0), 0);
        assert_eq!(count_scalars(&text), 1);
        assert_eq!(prefix_end(&text, 0), 0);
        assert_eq!(prefix_end(&text, 1), 3);
        assert_eq!(prefix_end(&text, usize::MAX), 3);
    }

    #[test]
    fn all_encoded_lengths_and_ordinary_codepoints() {
        let mut text = L!("a😀e\u{301}").to_owned();
        text.push_utfstr(&bytes(&[
            0xc2, 0xa2, 0xef, 0x98, 0x80, 0xf0, 0x9f, 0x98, 0x80,
        ]));
        let boundaries = [0, 1, 2, 3, 4, 6, 9, 13];
        assert_eq!(count_scalars(&text), boundaries.len() - 1);
        for (count, &pos) in boundaries.iter().enumerate() {
            assert_eq!(prefix_end(&text, count), pos);
            if count > 0 {
                assert_eq!(previous_boundary(&text, pos), boundaries[count - 1]);
            }
            if count + 1 < boundaries.len() {
                assert_eq!(next_boundary(&text, pos), boundaries[count + 1]);
            }
        }
    }

    #[test]
    fn malformed_escaped_bytes_stay_byte_wise() {
        for raw in [
            &[0xff, 0x80, 0xc0, 0xaf][..], // invalid leads and overlong two-byte encoding
            &[0xe0, 0x80, 0x80][..],       // overlong three-byte encoding
            &[0xed, 0xa0, 0x80][..],       // UTF-16 surrogate
            &[0xf4, 0x90, 0x80, 0x80][..], // above U+10FFFF
            &[0xf0, 0x9f, 0x98][..],       // incomplete scalar
        ] {
            let text = bytes(raw);
            assert_eq!(count_scalars(&text), raw.len());
            for pos in 0..=text.len() {
                assert_eq!(floor_boundary(&text, pos), pos);
                assert_eq!(ceil_boundary(&text, pos), pos);
                assert_eq!(prefix_end(&text, pos), pos);
            }
        }
        // A malformed lead must not hide a complete valid scalar that follows it.
        let text = bytes(&[0xe0, 0xc2, 0xa2, 0xff]);
        assert_eq!(count_scalars(&text), 3);
        assert_eq!(prefix_end(&text, 2), 3);
        assert_eq!(floor_boundary(&text, 2), 1);
    }
}
