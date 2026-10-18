use std::ops::Range;

use crate::highlight::HighlightSpec;
use crate::prelude::*;

/// An edit action that can be undone.
#[derive(Clone, PartialEq)]
pub struct Edit {
    /// When undoing the edit we use this to restore the previous cursor position.
    pub cursor_position_before_edit: usize,
    pub cursor_position_before_undo: Option<usize>,

    /// The span of text that is replaced by this edit.
    pub range: std::ops::Range<usize>,

    /// The strings that are removed and added by this edit, respectively.
    pub old: WString,
    pub replacement: WString,

    /// edit_t is only for contiguous changes, so to restore a group of arbitrary changes to the
    /// command line we need to have a group ID as forcibly coalescing changes is not enough.
    group_id: Option<usize>,
}

impl Edit {
    pub fn new(range: std::ops::Range<usize>, replacement: WString) -> Self {
        Self {
            cursor_position_before_edit: 0,
            cursor_position_before_undo: None,
            range,
            old: WString::new(),
            replacement,
            group_id: None,
        }
    }
}

/// Modify a string and its syntax highlighting according to the given edit.
/// Currently exposed for testing only.
pub fn apply_edit(target: &mut WString, colors: &mut Vec<HighlightSpec>, edit: &Edit) {
    let range = &edit.range;
    target.replace_range(range.clone(), &edit.replacement);

    // Now do the same to highlighting.
    let last_color = edit
        .range
        .start
        .checked_sub(1)
        .map(|i| colors[i])
        .unwrap_or_default();
    colors.splice(
        range.clone(),
        std::iter::repeat_n(last_color, edit.replacement.len()),
    );
}

/// Apply a model edit and maintain the native scalar count across both splice seams.
/// Three neighboring characters suffice because encoded UTF-8 scalars occupy at most four.
fn apply_line_edit(
    target: &mut WString,
    colors: &mut Vec<HighlightSpec>,
    edit: &Edit,
    #[cfg(windows)] scalar_count: &mut usize,
) {
    #[cfg(windows)]
    let (start, end, removed_count) = {
        use crate::native_text::{ceil_boundary, count_scalars, floor_boundary};
        let start = floor_boundary(target, edit.range.start.saturating_sub(3));
        let end = ceil_boundary(target, edit.range.end.saturating_add(3).min(target.len()));
        (start, end, count_scalars(&target[start..end]))
    };
    apply_edit(target, colors, edit);
    #[cfg(windows)]
    {
        let end = end - edit.range.len() + edit.replacement.len();
        *scalar_count =
            *scalar_count - removed_count + crate::native_text::count_scalars(&target[start..end]);
    }
}

fn is_single_editing_scalar(text: &wstr) -> bool {
    #[cfg(windows)]
    {
        !text.is_empty() && crate::native_text::next_boundary(text, 0) == text.len()
    }
    #[cfg(not(windows))]
    {
        text.len() == 1
    }
}

/// The history of all edits to some command line.
#[derive(Clone, Default)]
pub struct UndoHistory {
    /// The stack of edits that can be undone or redone atomically.
    pub edits: Vec<Edit>,

    /// The position in the undo stack that corresponds to the current
    /// state of the input line.
    /// Invariants:
    ///     edits_applied - 1 is the index of the next edit to undo.
    ///     edits_applied     is the index of the next edit to redo.
    ///
    /// For example, if nothing was undone, edits_applied is edits.size().
    /// If every single edit was undone, edits_applied is 0.
    pub edits_applied: usize,

    /// Whether we allow the next edit to be grouped together with the
    /// last one.
    may_coalesce: bool,

    /// Whether to be more aggressive in coalescing edits. Ideally, it would be "force coalesce"
    /// with guaranteed atomicity but as `edit_t` is strictly for contiguous changes, that guarantee
    /// can't be made at this time.
    try_coalesce: bool,
}

impl UndoHistory {
    /// Empty the history.
    pub fn clear(&mut self) {
        self.edits.clear();
        self.edits_applied = 0;
        self.may_coalesce = false;
    }
}

/// Helper class for storing a command line.
#[derive(Clone, Default)]
pub struct EditableLine {
    /// The command line.
    text: WString,
    /// Number of native editing scalars, maintained locally at each edit's splice seams.
    #[cfg(windows)]
    scalar_count: usize,
    /// Syntax highlighting.
    colors: Vec<HighlightSpec>,
    /// The current position of the cursor in the command line.
    position: usize,
    pub pending_position: Option<usize>,

    /// The history of all edits.
    undo_history: UndoHistory,
    /// The nesting level for atomic edits, so that recursive invocations of start_edit_group()
    /// are not ended by one end_edit_group() call.
    edit_group_level: Option<usize>,
    /// Monotonically increasing edit group, ignored when edit_group_level_ is -1. Allowed to wrap.
    edit_group_id: usize,
}

impl EditableLine {
    pub fn text(&self) -> &wstr {
        &self.text
    }

    pub fn colors(&self) -> &[HighlightSpec] {
        &self.colors
    }
    pub fn set_colors(&mut self, colors: Vec<HighlightSpec>) {
        assert_eq!(colors.len(), self.len());
        self.colors = colors;
    }

    pub fn position(&self) -> usize {
        self.position
    }
    pub fn set_position(&mut self, position: usize) {
        assert!(position <= self.len());
        #[cfg(windows)]
        let position = self.native_position(position);
        self.position = position;
    }

    // Gets the length of the text.
    pub fn len(&self) -> usize {
        self.text.len()
    }

    /// Number of editing scalars, not the number of canonical shell-text indices.
    pub fn char_count(&self) -> usize {
        #[cfg(windows)]
        {
            self.scalar_count
        }
        #[cfg(unix)]
        {
            self.len()
        }
    }

    /// Snap an admitted cursor position in the direction of movement.
    #[cfg(windows)]
    fn native_position(&self, position: usize) -> usize {
        if position < self.position {
            crate::native_text::floor_boundary(&self.text, position)
        } else {
            crate::native_text::ceil_boundary(&self.text, position)
        }
    }

    fn set_position_after_edit(&mut self, position: usize) {
        #[cfg(windows)]
        let position = crate::native_text::ceil_boundary(&self.text, position);
        self.set_position(position);
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn at(&self, idx: usize) -> char {
        self.text.char_at(idx)
    }

    pub fn offset_to_line(&self, offset: usize) -> usize {
        self.text[0..offset].chars().filter(|&c| c == '\n').count()
    }

    /// Modify the commandline according to @edit. Most modifications to the
    /// text should pass through this function.
    pub fn push_edit(&mut self, mut edit: Edit, allow_coalesce: bool) {
        #[cfg(windows)]
        {
            use crate::native_text::{ceil_boundary, floor_boundary};
            assert!(edit.range.start <= edit.range.end && edit.range.end <= self.len());
            edit.range = if edit.range.is_empty() {
                let position = self.native_position(edit.range.start);
                position..position
            } else {
                floor_boundary(&self.text, edit.range.start)
                    ..ceil_boundary(&self.text, edit.range.end)
            };
        }
        let range = &edit.range;
        let is_insertion = range.is_empty();
        #[cfg(windows)]
        let allow_coalesce = allow_coalesce && range.start == self.position();
        // Coalescing insertion does not create a new undo entry but adds to the last insertion.
        if allow_coalesce && is_insertion && self.want_to_coalesce_insertion_of(&edit.replacement) {
            assert_eq!(range.start, self.position());
            let last_edit = self.undo_history.edits.last_mut().unwrap();
            last_edit.replacement.push_utfstr(&edit.replacement);
            apply_line_edit(
                &mut self.text,
                &mut self.colors,
                &edit,
                #[cfg(windows)]
                &mut self.scalar_count,
            );
            self.set_position_after_edit(self.position() + edit.replacement.len());

            assert!(self.undo_history.may_coalesce);
            return;
        }

        if range.is_empty() && edit.replacement.is_empty() {
            return; // nop
        }

        // Assign a new group ID or propagate the old one if we're in a logical grouping of edits
        if self.edit_group_level.is_some() {
            edit.group_id = Some(self.edit_group_id);
        }

        if self.undo_history.edits_applied != self.undo_history.edits.len() {
            // After undoing some edits, the user is making a new edit;
            // we are about to create a new edit branch.
            // Discard all edits that were undone because we only support
            // linear undo/redo, they will be unreachable.
            self.undo_history
                .edits
                .truncate(self.undo_history.edits_applied);
        }
        let position = self.pending_position.take().unwrap_or(self.position());
        #[cfg(windows)]
        let position = self.native_position(position);
        edit.cursor_position_before_edit = position;
        edit.old = self.text[range.clone()].to_owned();
        apply_line_edit(
            &mut self.text,
            &mut self.colors,
            &edit,
            #[cfg(windows)]
            &mut self.scalar_count,
        );
        self.set_position_after_edit(cursor_position_after_edit(&edit));
        assert_eq!(
            self.undo_history.edits_applied,
            self.undo_history.edits.len()
        );
        self.undo_history.may_coalesce = is_insertion
            && (self.undo_history.try_coalesce || is_single_editing_scalar(&edit.replacement));
        self.undo_history.edits_applied += 1;
        self.undo_history.edits.push(edit);
    }

    /// Undo the most recent edit that was not yet undone. Returns true on success.
    pub fn undo(&mut self) -> bool {
        let mut did_undo = false;
        let mut last_group_id = None;
        let position_before_undo = self.position();
        let end = self.undo_history.edits_applied;
        while self.undo_history.edits_applied != 0 {
            let edit = &self.undo_history.edits[self.undo_history.edits_applied - 1];
            if did_undo
                && edit
                    .group_id
                    .is_none_or(|group_id| Some(group_id) != last_group_id)
            {
                // We've restored all the edits in this logical undo group
                break;
            }
            last_group_id = edit.group_id;
            self.undo_history.edits_applied -= 1;
            let range = &edit.range;
            let mut inverse = Edit::new(
                range.start..range.start + edit.replacement.len(),
                L!("").to_owned(),
            );
            inverse.replacement = edit.old.clone();
            let old_position = edit.cursor_position_before_edit;
            apply_line_edit(
                &mut self.text,
                &mut self.colors,
                &inverse,
                #[cfg(windows)]
                &mut self.scalar_count,
            );
            self.set_position(old_position);
            did_undo = true;
        }
        if did_undo {
            let edit = &mut self.undo_history.edits[end - 1];
            edit.cursor_position_before_undo = Some(position_before_undo);
        }

        self.end_edit_group();
        self.undo_history.may_coalesce = false;
        did_undo
    }

    /// Redo the most recent undo. Returns true on success.
    pub fn redo(&mut self) -> bool {
        let mut did_redo = false;

        let mut last_group_id = None;
        while let Some(edit) = self.undo_history.edits.get(self.undo_history.edits_applied) {
            if did_redo
                && edit
                    .group_id
                    .is_none_or(|group_id| Some(group_id) != last_group_id)
            {
                // We've restored all the edits in this logical undo group
                break;
            }
            last_group_id = edit.group_id;
            self.undo_history.edits_applied += 1;
            apply_line_edit(
                &mut self.text,
                &mut self.colors,
                edit,
                #[cfg(windows)]
                &mut self.scalar_count,
            );
            edit.cursor_position_before_undo
                .map(|pos| self.set_position(pos));
            did_redo = true;
        }

        self.end_edit_group();
        did_redo
    }

    /// Start a logical grouping of command line edits that should be undone/redone together.
    pub fn begin_edit_group(&mut self) {
        if self.edit_group_level.is_some() {
            return;
        }
        self.edit_group_level = Some(1);
        // Indicate that the next change must trigger the creation of a new history item
        self.undo_history.may_coalesce = false;
        // Indicate that future changes should be coalesced into the same edit if possible.
        self.undo_history.try_coalesce = true;
        // Assign a logical edit group ID to future edits in this group
        self.edit_group_id += 1;
    }

    /// End a logical grouping of command line edits that should be undone/redone together.
    pub fn end_edit_group(&mut self) {
        match self.edit_group_level.as_mut() {
            Some(edit_group_level) => {
                *edit_group_level -= 1;
                if *edit_group_level > 0 {
                    return;
                }
            }
            None => {
                // Prevent unbalanced end_edit_group() calls from breaking everything.
                return;
            }
        }

        self.edit_group_level = None;
        self.undo_history.try_coalesce = false;
        self.undo_history.may_coalesce = false;
    }

    /// Whether we want to append this string to the previous edit.
    fn want_to_coalesce_insertion_of(&self, s: &wstr) -> bool {
        // The previous edit must support coalescing.
        if !self.undo_history.may_coalesce {
            return false;
        }
        // Only consolidate single character inserts.
        if !is_single_editing_scalar(s) {
            return false;
        }
        // Make an undo group after every space.
        if s.as_char_slice()[0] == ' ' && !self.undo_history.try_coalesce {
            return false;
        }
        let last_edit = self.undo_history.edits.last().unwrap();
        // Don't add to the last edit if it deleted something.
        if !last_edit.range.is_empty() {
            return false;
        }
        #[cfg(windows)]
        if last_edit.range.start + last_edit.replacement.len() != self.position() {
            // An insertion away from the cursor must not absorb a later insertion at the cursor.
            return false;
        }
        // Must not have moved the cursor!
        if cursor_position_after_edit(last_edit) != self.position() {
            return false;
        }
        true
    }
}

/// Returns the number of characters left of the cursor that are removed by the
/// deletion in the given edit.
#[cfg(not(windows))]
fn chars_deleted_left_of_cursor(edit: &Edit) -> usize {
    if edit.cursor_position_before_edit > edit.range.start {
        return std::cmp::min(
            edit.range.len(),
            edit.cursor_position_before_edit - edit.range.start,
        );
    }
    0
}

/// Compute the position of the cursor after the given edit.
fn cursor_position_after_edit(edit: &Edit) -> usize {
    #[cfg(windows)]
    {
        let position = edit.cursor_position_before_edit;
        return if position < edit.range.start {
            position
        } else if position <= edit.range.end {
            edit.range.start + edit.replacement.len()
        } else {
            position - edit.range.len() + edit.replacement.len()
        };
    }
    #[cfg(not(windows))]
    {
        let cursor = edit.cursor_position_before_edit + edit.replacement.len();
        let removed = chars_deleted_left_of_cursor(edit);
        cursor.saturating_sub(removed)
    }
}

pub fn range_of_line_at_cursor(buffer: &wstr, cursor: usize) -> Range<usize> {
    let start = buffer[0..cursor]
        .as_char_slice()
        .iter()
        .rposition(|&c| c == '\n')
        .map_or(0, |newline| newline + 1);
    let mut end = buffer[cursor..]
        .as_char_slice()
        .iter()
        .position(|&c| c == '\n')
        .map_or(buffer.len(), |pos| cursor + pos);
    // Remove any trailing newline
    if end != start && buffer.char_at(end - 1) == '\n' {
        end -= 1;
    }
    start..end
}

pub fn line_at_cursor(buffer: &wstr, cursor: usize) -> &wstr {
    &buffer[range_of_line_at_cursor(buffer, cursor)]
}

#[cfg(test)]
mod tests {
    use crate::{
        editable_line::{Edit, EditableLine},
        prelude::*,
    };

    #[test]
    fn test_undo() {
        let mut line = EditableLine::default();

        let insert = |line: &EditableLine| line.position()..line.position();

        assert!(!line.undo()); // nothing to undo
        assert!(line.text().is_empty());
        assert_eq!(line.position(), 0);
        line.push_edit(Edit::new(0..0, L!("a b c").to_owned()), true);
        assert_eq!(line.text(), L!("a b c").to_owned());
        assert_eq!(line.position(), 5);
        line.set_position(2);
        line.push_edit(Edit::new(2..3, L!("B").to_owned()), true); // replacement right of cursor
        assert_eq!(line.text(), L!("a B c").to_owned());
        line.undo();
        assert_eq!(line.text(), L!("a b c").to_owned());
        assert_eq!(line.position(), 2);
        line.redo();
        assert_eq!(line.text(), L!("a B c").to_owned());
        assert_eq!(line.position(), 3);

        assert!(!line.redo()); // nothing to redo

        line.push_edit(Edit::new(0..2, L!("").to_owned()), true); // deletion left of cursor
        assert_eq!(line.text(), L!("B c").to_owned());
        assert_eq!(line.position(), 1);
        line.undo();
        assert_eq!(line.text(), L!("a B c").to_owned());
        assert_eq!(line.position(), 3);
        line.redo();
        assert_eq!(line.text(), L!("B c").to_owned());
        assert_eq!(line.position(), 1);

        line.push_edit(Edit::new(0..line.len(), L!("a b c").to_owned()), true); // replacement left and right of cursor
        assert_eq!(line.text(), L!("a b c").to_owned());
        assert_eq!(line.position(), 5);

        // Undo coalesced edits
        line.push_edit(Edit::new(0..line.len(), L!("").to_owned()), false);
        line.push_edit(Edit::new(insert(&line), L!("a").to_owned()), true);
        line.push_edit(Edit::new(insert(&line), L!("b").to_owned()), true);
        line.push_edit(Edit::new(insert(&line), L!("c").to_owned()), true);
        line.push_edit(Edit::new(insert(&line), L!(" ").to_owned()), true);
        line.undo();
        line.undo();
        line.redo();
        assert_eq!(line.text(), L!("abc").to_owned());
        // This removes the space insertion from the history, but does not coalesce with the first edit.
        line.push_edit(Edit::new(insert(&line), L!("d").to_owned()), true);
        line.push_edit(Edit::new(insert(&line), L!("e").to_owned()), true);
        assert_eq!(line.text(), L!("abcde").to_owned());
        line.undo();
        assert_eq!(line.text(), L!("abc").to_owned());
    }

    #[test]
    fn test_undo_group() {
        let mut line = EditableLine::default();
        line.begin_edit_group();
        line.push_edit(Edit::new(0..0, L!("a").to_owned()), true);
        line.end_edit_group();
        line.begin_edit_group();
        line.push_edit(Edit::new(1..1, L!("b").to_owned()), true);
        line.end_edit_group();
        line.undo();
        assert_eq!(line.text(), "a");
    }

    #[cfg(windows)]
    fn encoded_bytes(bytes: &[u8]) -> WString {
        bytes
            .iter()
            .copied()
            .map(fish_widestring::encode_byte_to_char)
            .collect()
    }

    #[cfg(windows)]
    fn reserved_line() -> EditableLine {
        let mut line = EditableLine::default();
        let mut text = L!("a").to_owned();
        text.push_utfstr(&encoded_bytes(&[0xef, 0x98, 0x80])); // U+F600
        text.push('z');
        line.push_edit(Edit::new(0..0, text), false);
        assert_eq!(line.len(), 5);
        assert_eq!(line.char_count(), 3);
        line
    }

    #[cfg(windows)]
    #[test]
    fn native_reserved_scalar_backspace_and_delete() {
        for backwards in [true, false] {
            let mut line = reserved_line();
            line.set_position(if backwards { 4 } else { 1 });
            // Even a caller still supplying a one-index edit cannot split the scalar.
            let range = if backwards { 3..4 } else { 1..2 };
            line.push_edit(Edit::new(range, WString::new()), false);
            assert_eq!(line.text(), L!("az"));
            assert_eq!(line.position(), 1);
            assert_eq!(line.char_count(), 2);
            assert!(line.undo());
            assert_eq!(line.text(), reserved_line().text());
            assert_eq!(line.position(), if backwards { 4 } else { 1 });
            assert_eq!(line.char_count(), 3);
            assert!(line.redo());
            assert_eq!(line.text(), L!("az"));
            assert_eq!(line.position(), 1);
            assert_eq!(line.char_count(), 2);
        }
    }

    #[cfg(windows)]
    #[test]
    fn native_cursor_admission_and_selection_replacement() {
        let mut line = reserved_line();
        line.set_position(0);
        line.set_position(2); // rightward movement crosses the whole encoded scalar
        assert_eq!(line.position(), 4);
        line.set_position(3); // leftward movement crosses it in the opposite direction
        assert_eq!(line.position(), 1);
        line.set_position(4);
        line.push_edit(Edit::new(2..3, L!("Q").to_owned()), false);
        assert_eq!(line.text(), L!("aQz"));
        assert_eq!(line.position(), 2);
        assert_eq!(line.char_count(), 3);
        assert!(line.undo());
        assert_eq!(line.text(), reserved_line().text());
        assert_eq!(line.position(), 4);
        assert_eq!(line.char_count(), 3);
        assert!(line.redo());
        assert_eq!(line.text(), L!("aQz"));
        assert_eq!(line.position(), 2);

        // Expanding a selection must not advance a cursor before the selection.
        let mut line = reserved_line();
        line.set_position(0);
        line.push_edit(Edit::new(2..3, L!("Q").to_owned()), false);
        assert_eq!(line.text(), L!("aQz"));
        assert_eq!(line.position(), 0);
        assert!(line.undo());
        assert_eq!(line.position(), 0);
        assert!(line.redo());
        assert_eq!(line.position(), 0);

        // A cursor after the selection tracks the unchanged suffix, not the old indices.
        let mut line = reserved_line();
        line.push_edit(Edit::new(2..3, L!("Q").to_owned()), false);
        assert_eq!(line.text(), L!("aQz"));
        assert_eq!(line.position(), 3);
    }

    #[cfg(windows)]
    #[test]
    fn native_insertion_admission_and_scalar_coalescing() {
        let mut line = reserved_line();
        line.set_position(1);
        line.push_edit(Edit::new(2..2, L!("!").to_owned()), true);
        let mut expected = L!("a").to_owned();
        expected.push_utfstr(&encoded_bytes(&[0xef, 0x98, 0x80]));
        expected.push_utfstr(L!("!z"));
        assert_eq!(line.text(), &expected);
        assert_eq!(line.position(), 1); // insertion was beyond the cursor
        assert_eq!(line.char_count(), 4);
        line.push_edit(Edit::new(1..1, L!("x").to_owned()), true);
        assert_eq!(line.position(), 2);
        assert_eq!(line.char_count(), 5);
        assert!(line.undo());
        assert_eq!(line.text(), &expected);
        assert_eq!(line.position(), 1);
        assert_eq!(line.char_count(), 4);
        assert!(line.undo());
        assert_eq!(line.text(), reserved_line().text());

        let mut line = EditableLine::default();
        line.push_edit(Edit::new(0..0, encoded_bytes(&[0xef, 0x98, 0x80])), true);
        line.push_edit(Edit::new(3..3, L!("x").to_owned()), true);
        assert_eq!(line.position(), 4);
        assert_eq!(line.char_count(), 2);
        assert!(line.undo());
        assert!(line.is_empty());
        assert_eq!(line.char_count(), 0);
        assert!(line.redo());
        assert_eq!(line.position(), 4);
        assert_eq!(line.char_count(), 2);
    }

    #[cfg(windows)]
    #[test]
    fn native_count_tracks_bytes_joining_and_separating_at_splice_seams() {
        let mut line = EditableLine::default();
        for (byte, count) in [(0xef, 1), (0x98, 2), (0x80, 1)] {
            let pos = line.position();
            line.push_edit(Edit::new(pos..pos, encoded_bytes(&[byte])), false);
            assert_eq!(line.char_count(), count);
        }
        assert_eq!(line.position(), 3);
        assert!(line.undo());
        assert_eq!(line.text(), &encoded_bytes(&[0xef, 0x98]));
        assert_eq!(line.position(), 2);
        assert_eq!(line.char_count(), 2);
        assert!(line.redo());
        assert_eq!(line.text(), &encoded_bytes(&[0xef, 0x98, 0x80]));
        assert_eq!(line.position(), 3);
        assert_eq!(line.char_count(), 1);

        // Deleting ASCII can join previously malformed bytes into a complete scalar.
        let mut text = encoded_bytes(&[0xef]);
        text.push('x');
        text.push_utfstr(&encoded_bytes(&[0x98, 0x80, 0xff]));
        line.push_edit(Edit::new(0..line.len(), text), false);
        assert_eq!(line.char_count(), 5);
        line.set_position(1);
        line.push_edit(Edit::new(1..2, WString::new()), false);
        assert_eq!(line.char_count(), 2);
        assert_eq!(line.position(), 3); // admitted at the end of the newly joined scalar
        assert!(line.undo());
        assert_eq!(line.char_count(), 5);
        assert_eq!(line.position(), 1);
        assert!(line.redo());
        assert_eq!(line.char_count(), 2);
        assert_eq!(line.position(), 3);

        // The remaining invalid escaped byte is still independently deletable.
        line.set_position(4);
        line.push_edit(Edit::new(3..4, WString::new()), false);
        assert_eq!(line.char_count(), 1);
        assert_eq!(line.position(), 3);
    }
}
