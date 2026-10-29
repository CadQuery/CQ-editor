//! Pure text operations behind the editor commands.
//!
//! Positions are byte offsets into the text. A selection is an
//! `(anchor, cursor)` pair; both are remapped through every edit so the
//! selection keeps covering the same text afterwards.

use std::ops::Range;

pub const INDENT: &str = "    ";
const INDENT_WIDTH: usize = 4;

/// Statements after which the next line is usually dedented.
const DEDENT_KEYWORDS: &[&str] = &["return", "pass", "break", "continue", "raise"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    pub anchor: usize,
    pub cursor: usize,
}

impl Selection {
    pub fn caret(at: usize) -> Self {
        Self {
            anchor: at,
            cursor: at,
        }
    }

    pub fn range(&self) -> Range<usize> {
        self.anchor.min(self.cursor)..self.anchor.max(self.cursor)
    }
}

/// The result of an edit: the new text and the remapped selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edited {
    pub text: String,
    pub selection: Selection,
}

pub fn char_to_byte(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map_or(text.len(), |(i, _)| i)
}

pub fn byte_to_char(text: &str, byte_index: usize) -> usize {
    text[..byte_index.min(text.len())].chars().count()
}

/// 0-based line containing `byte`.
pub fn line_of(text: &str, byte: usize) -> usize {
    text.as_bytes()[..byte.min(text.len())]
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
}

/// Byte offset where 0-based `line` starts (the end of the text past the last line).
pub fn line_start(text: &str, line: usize) -> usize {
    if line == 0 {
        return 0;
    }
    text.match_indices('\n')
        .nth(line - 1)
        .map_or(text.len(), |(i, _)| i + 1)
}

/// Byte range of 0-based `line`, without its line break.
pub fn line_range(text: &str, line: usize) -> Range<usize> {
    let start = line_start(text, line);
    let end = text[start..].find('\n').map_or(text.len(), |i| start + i);
    start..end
}

/// 0-based line and character column of `byte`.
pub fn line_column(text: &str, byte: usize) -> (usize, usize) {
    let line = line_of(text, byte);
    let start = line_start(text, line);
    (line, text[start..byte.min(text.len())].chars().count())
}

/// Follows 1-based line numbers (breakpoints, the error line) through an
/// edit, so they stay on their statements when lines are added or removed.
///
/// The edit is taken as one block of replaced lines between the lines both
/// texts share at the start and at the end, which is what typing, pasting,
/// the editor commands and reloads produce. Lines before the block keep
/// their number, lines after it shift, and lines inside it map by position
/// within the block; a block deleted entirely takes its lines with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlockMove {
    None,
    /// The first line of the replaced block moved to its end (the other lines moved up by 1).
    FirstToLast,
    /// The last line of the replaced block moved to its start (the other lines moved down by 1).
    LastToFirst,
}

pub struct LineMap {
    /// Unchanged leading lines.
    prefix: usize,
    /// Length of the replaced block in the old text, in lines.
    old_count: usize,
    /// Length of the block that replaced it, in lines.
    new_count: usize,
    /// Whether the replaced block is a one-line rotation (Move Line Up / Down).
    moved: BlockMove,
}

impl LineMap {
    pub fn new(old: &str, new: &str) -> Self {
        let old_lines: Vec<&str> = old.split('\n').collect();
        let new_lines: Vec<&str> = new.split('\n').collect();
        let prefix = old_lines
            .iter()
            .zip(&new_lines)
            .take_while(|(a, b)| a == b)
            .count();
        let room = old_lines.len().min(new_lines.len()) - prefix;
        let suffix = old_lines
            .iter()
            .rev()
            .zip(new_lines.iter().rev())
            .take(room)
            .take_while(|(a, b)| a == b)
            .count();
        let old_count = old_lines.len() - prefix - suffix;
        let new_count = new_lines.len() - prefix - suffix;
        let old_block = &old_lines[prefix..prefix + old_count];
        let new_block = &new_lines[prefix..prefix + new_count];
        let moved = if old_count == new_count && old_count >= 2 {
            if old_block[1..] == new_block[..old_count - 1]
                && old_block[0] == new_block[old_count - 1]
            {
                BlockMove::FirstToLast
            } else if old_block[..old_count - 1] == new_block[1..]
                && old_block[old_count - 1] == new_block[0]
            {
                BlockMove::LastToFirst
            } else {
                BlockMove::None
            }
        } else {
            BlockMove::None
        };
        Self {
            prefix,
            old_count,
            new_count,
            moved,
        }
    }

    /// Where 1-based `line` of the old text is now; `None` if it was deleted.
    pub fn map(&self, line: usize) -> Option<usize> {
        let index = line.checked_sub(1)?;
        if index < self.prefix {
            return Some(line);
        }
        let offset = index - self.prefix;
        if offset >= self.old_count {
            return Some(line - self.old_count + self.new_count);
        }
        if self.new_count == 0 {
            return None;
        }
        match self.moved {
            BlockMove::FirstToLast => {
                let new_offset = if offset == 0 {
                    self.old_count - 1
                } else {
                    offset - 1
                };
                return Some(self.prefix + new_offset + 1);
            }
            BlockMove::LastToFirst => {
                let new_offset = if offset + 1 == self.old_count {
                    0
                } else {
                    offset + 1
                };
                return Some(self.prefix + new_offset + 1);
            }
            BlockMove::None => {}
        }
        // Merged lines end up on the last line of the new block.
        Some(self.prefix + offset.min(self.new_count - 1) + 1)
    }
}

/// Lines touched by a selection. A selection ending at the very start of a
/// line does not include that line.
pub fn selected_lines(text: &str, selection: &Selection) -> Range<usize> {
    let range = selection.range();
    let first = line_of(text, range.start);
    let mut last = line_of(text, range.end);
    if last > first && range.end == line_start(text, last) {
        last -= 1;
    }
    first..last + 1
}

/// Applies non-overlapping replacements, sorted by position.
fn apply(text: &str, selection: &Selection, edits: &[(Range<usize>, String)]) -> Edited {
    let mut out = String::with_capacity(text.len() + edits.len() * INDENT_WIDTH);
    let mut last = 0;
    for (range, replacement) in edits {
        out.push_str(&text[last..range.start]);
        out.push_str(replacement);
        last = range.end;
    }
    out.push_str(&text[last..]);

    let map = |offset: usize| {
        let mut shift: isize = 0;
        for (range, replacement) in edits {
            if offset >= range.end {
                shift += replacement.len() as isize - range.len() as isize;
            } else if offset > range.start {
                // Inside a removed span: snap to its start.
                shift -= (offset - range.start) as isize;
            }
        }
        (offset as isize + shift).max(0) as usize
    };
    Edited {
        text: out,
        selection: Selection {
            anchor: map(selection.anchor),
            cursor: map(selection.cursor),
        },
    }
}

fn leading_whitespace(line: &str) -> &str {
    &line[..line.len() - line.trim_start().len()]
}

/// Comments the selected lines with `# `, or uncomments them when every
/// non-blank line is already a comment.
pub fn toggle_comment(text: &str, selection: &Selection) -> Edited {
    let lines: Vec<Range<usize>> = selected_lines(text, selection)
        .map(|line| line_range(text, line))
        .filter(|range| !text[range.clone()].trim().is_empty())
        .collect();
    if lines.is_empty() {
        return Edited {
            text: text.to_owned(),
            selection: selection.clone(),
        };
    }

    let all_commented = lines
        .iter()
        .all(|range| text[range.clone()].trim_start().starts_with('#'));
    let edits: Vec<(Range<usize>, String)> = if all_commented {
        lines
            .iter()
            .map(|range| {
                let line = &text[range.clone()];
                let hash = range.start + leading_whitespace(line).len();
                let end = if text[hash + 1..].starts_with(' ') {
                    hash + 2
                } else {
                    hash + 1
                };
                (hash..end, String::new())
            })
            .collect()
    } else {
        let column = lines
            .iter()
            .map(|range| leading_whitespace(&text[range.clone()]).len())
            .min()
            .unwrap_or(0);
        lines
            .iter()
            .map(|range| (range.start + column..range.start + column, "# ".to_owned()))
            .collect()
    };
    apply(text, selection, &edits)
}

/// Tab: indents every selected line when the selection spans several lines,
/// otherwise replaces the selection with spaces up to the next tab stop.
pub fn indent(text: &str, selection: &Selection) -> Edited {
    let lines = selected_lines(text, selection);
    if lines.len() > 1 {
        let edits: Vec<_> = lines
            .map(|line| line_range(text, line))
            .filter(|range| !range.is_empty())
            .map(|range| (range.start..range.start, INDENT.to_owned()))
            .collect();
        return apply(text, selection, &edits);
    }

    let range = selection.range();
    let (_, column) = line_column(text, range.start);
    let spaces = INDENT_WIDTH - column % INDENT_WIDTH;
    let mut edited = apply(text, selection, &[(range.clone(), " ".repeat(spaces))]);
    edited.selection = Selection::caret(range.start + spaces);
    edited
}

/// Shift+Tab: removes one level of indentation from the selected lines.
pub fn unindent(text: &str, selection: &Selection) -> Edited {
    let edits: Vec<_> = selected_lines(text, selection)
        .map(|line| line_range(text, line))
        .filter_map(|range| {
            let line = &text[range.clone()];
            let remove = if line.starts_with('\t') {
                1
            } else {
                let spaces = line.len() - line.trim_start_matches(' ').len();
                match spaces {
                    0 => return None,
                    n if n % INDENT_WIDTH == 0 => INDENT_WIDTH,
                    n => n % INDENT_WIDTH,
                }
            };
            Some((range.start..range.start + remove, String::new()))
        })
        .collect();
    apply(text, selection, &edits)
}

/// Line break plus indentation for Enter at `cursor`: keeps the current
/// indentation, adds a level after a trailing `:` and removes one after
/// `return`, `pass` and similar.
pub fn newline_with_indent(text: &str, cursor: usize) -> String {
    let line = line_of(text, cursor);
    let before_cursor = &text[line_start(text, line)..cursor];
    let indent = leading_whitespace(before_cursor);
    let code = strip_comment(before_cursor).trim_end();

    let mut new_indent = indent.to_owned();
    if code.ends_with(':') {
        new_indent.push_str(INDENT);
    } else {
        let first_word = code
            .trim_start()
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .next();
        if first_word.is_some_and(|w| DEDENT_KEYWORDS.contains(&w)) {
            let keep = new_indent.len().saturating_sub(INDENT_WIDTH);
            new_indent.truncate(keep);
        }
    }
    format!("\n{new_indent}")
}

/// Replaces every one of the non-overlapping, sorted `ranges` with
/// `replacement` in one edit.
pub fn replace_ranges(
    text: &str,
    selection: &Selection,
    ranges: &[Range<usize>],
    replacement: &str,
) -> Edited {
    let edits: Vec<(Range<usize>, String)> = ranges
        .iter()
        .map(|range| (range.clone(), replacement.to_owned()))
        .collect();
    apply(text, selection, &edits)
}

/// Duplicates the current line or the selected block of lines immediately
/// below, moving the caret or selection onto the duplicated copy.
pub fn duplicate_lines(text: &str, selection: &Selection) -> Edited {
    let lines = selected_lines(text, selection);
    let block_start = line_start(text, lines.start);
    let block_end = line_range(text, lines.end - 1).end;
    let block = &text[block_start..block_end];
    let shift = block.len() + 1;

    let mut out = String::with_capacity(text.len() + shift);
    out.push_str(&text[..block_end]);
    out.push('\n');
    out.push_str(block);
    out.push_str(&text[block_end..]);

    Edited {
        selection: Selection {
            anchor: (selection.anchor + shift).min(out.len()),
            cursor: (selection.cursor + shift).min(out.len()),
        },
        text: out,
    }
}

/// Moves the current line or selected block of lines one line up, preserving
/// the selection within the moved block.
pub fn move_lines_up(text: &str, selection: &Selection) -> Edited {
    let lines = selected_lines(text, selection);
    if lines.start == 0 {
        return Edited {
            text: text.to_owned(),
            selection: selection.clone(),
        };
    }
    let above_range = line_range(text, lines.start - 1);
    let block_start = line_start(text, lines.start);
    let block_end = line_range(text, lines.end - 1).end;
    let delta = above_range.len() + 1;

    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..above_range.start]);
    out.push_str(&text[block_start..block_end]);
    out.push('\n');
    out.push_str(&text[above_range]);
    out.push_str(&text[block_end..]);

    Edited {
        selection: Selection {
            anchor: selection.anchor.saturating_sub(delta),
            cursor: selection.cursor.saturating_sub(delta),
        },
        text: out,
    }
}

/// Moves the current line or selected block of lines one line down, preserving
/// the selection within the moved block.
pub fn move_lines_down(text: &str, selection: &Selection) -> Edited {
    let lines = selected_lines(text, selection);
    let total_lines = text.matches('\n').count() + 1;
    if lines.end >= total_lines {
        return Edited {
            text: text.to_owned(),
            selection: selection.clone(),
        };
    }
    let block_start = line_start(text, lines.start);
    let block_end = line_range(text, lines.end - 1).end;
    let below_range = line_range(text, lines.end);
    let delta = below_range.len() + 1;

    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..block_start]);
    out.push_str(&text[below_range.clone()]);
    out.push('\n');
    out.push_str(&text[block_start..block_end]);
    out.push_str(&text[below_range.end..]);

    Edited {
        selection: Selection {
            anchor: (selection.anchor + delta).min(out.len()),
            cursor: (selection.cursor + delta).min(out.len()),
        },
        text: out,
    }
}

/// Inserts a code snippet at `selection`, adapting indentation to the target
/// line and adding `import cadquery as cq` when needed in a fresh script.
pub fn insert_snippet(text: &str, selection: &Selection, code: &str, needs_import: bool) -> Edited {
    let range = selection.range();
    let line = line_of(text, range.start);
    let line_span = line_range(text, line);
    let indent = leading_whitespace(&text[line_span.clone()]);
    let prepend_import = needs_import && !text.contains("import cadquery") && indent.is_empty();

    let body = if prepend_import {
        format!("import cadquery as cq\n\n{}", code.trim_end_matches('\n'))
    } else {
        code.trim_end_matches('\n').to_owned()
    };

    let indent_lines = |first_indented: bool| -> String {
        body.split('\n')
            .enumerate()
            .map(|(index, row)| {
                if row.is_empty() || (index == 0 && !first_indented) {
                    row.to_owned()
                } else {
                    format!("{indent}{row}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let (replace_range, inserted) =
        if range.is_empty() && !text[line_span.start..range.start].trim().is_empty() {
            (
                line_span.end..line_span.end,
                format!("\n{}", indent_lines(true)),
            )
        } else {
            (range, indent_lines(false))
        };

    let cursor = replace_range.start + inserted.len();
    let mut out = String::with_capacity(text.len() + inserted.len());
    out.push_str(&text[..replace_range.start]);
    out.push_str(&inserted);
    out.push_str(&text[replace_range.end..]);

    Edited {
        text: out,
        selection: Selection::caret(cursor),
    }
}

/// Characters of a Python name.
pub fn is_word_char(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

/// Byte range of the name containing the character at `byte`, if that
/// character is part of a name.
pub fn word_at(text: &str, byte: usize) -> Option<Range<usize>> {
    let c = text.get(byte..)?.chars().next()?;
    if !is_word_char(c) {
        return None;
    }
    let start = text[..byte]
        .char_indices()
        .rev()
        .take_while(|(_, c)| is_word_char(*c))
        .last()
        .map_or(byte, |(i, _)| i);
    let end = text[byte..]
        .char_indices()
        .find(|(_, c)| !is_word_char(*c))
        .map_or(text.len(), |(i, _)| byte + i);
    Some(start..end)
}

/// The part of a name typed just before `cursor`, when the cursor is at
/// the end of that name.
pub fn name_before(text: &str, cursor: usize) -> &str {
    let Some(before) = text.get(..cursor) else {
        return "";
    };
    if text[cursor..].chars().next().is_some_and(is_word_char) {
        return "";
    }
    let start = before
        .char_indices()
        .rev()
        .take_while(|(_, c)| is_word_char(*c))
        .last()
        .map_or(cursor, |(i, _)| i);
    &text[start..cursor]
}

/// Parses *Go to line* input: `line` or `line:column`, both 1-based.
/// Returns the 0-based line and the byte offset to jump to, clamped to the
/// text.
pub fn go_to_target(text: &str, input: &str) -> Option<(usize, usize)> {
    let (line, column) = match input.trim().split_once(':') {
        Some((line, column)) => (line.trim(), Some(column.trim())),
        None => (input.trim(), None),
    };
    let line: usize = line.parse().ok().filter(|&l| l > 0)?;
    let column: usize = match column {
        Some(column) => column.parse().ok().filter(|&c| c > 0)?,
        None => 1,
    };
    let last_line = text.matches('\n').count();
    let line = (line - 1).min(last_line);
    let range = line_range(text, line);
    let offset = text[range.clone()]
        .char_indices()
        .nth(column - 1)
        .map_or(range.end, |(i, _)| range.start + i);
    Some((line, offset))
}

/// Where a line of an earlier version of the text is now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Located {
    /// The same line, with the same text.
    Unchanged(usize),
    /// The same text, on another line.
    Moved(usize),
    /// Its text was edited; this is where the edits put the line.
    Changed(usize),
}

impl Located {
    pub fn line(self) -> usize {
        match self {
            Self::Unchanged(line) | Self::Moved(line) | Self::Changed(line) => line,
        }
    }
}

/// Where 1-based `line` of `old` is in `new`: on the line the edits moved
/// it to if its text is there, else on the nearest line with its text, else
/// where the edits put it. `None` if the line no longer exists.
pub fn locate_line(old: &str, line: usize, new: &str) -> Option<Located> {
    let content = old.split('\n').nth(line.checked_sub(1)?)?.trim();
    let lines: Vec<&str> = new.split('\n').map(str::trim).collect();
    let mapped = LineMap::new(old, new).map(line);
    if let Some(at) = mapped
        && lines.get(at - 1) == Some(&content)
    {
        return Some(if at == line {
            Located::Unchanged(at)
        } else {
            Located::Moved(at)
        });
    }
    if !content.is_empty() {
        let near = mapped.unwrap_or(line) - 1;
        let same = lines
            .iter()
            .enumerate()
            .filter(|(_, candidate)| **candidate == content)
            .min_by_key(|(index, _)| index.abs_diff(near));
        if let Some((index, _)) = same {
            return Some(if index + 1 == line {
                Located::Unchanged(line)
            } else {
                Located::Moved(index + 1)
            });
        }
    }
    mapped.map(Located::Changed)
}

/// Places breakpoints saved with the text of their lines (1-based line,
/// line text) in `text`: on the same line when its text is unchanged,
/// otherwise on the nearest line with that text, otherwise nowhere.
pub fn relocate_lines(text: &str, saved: &[(usize, String)]) -> std::collections::BTreeSet<usize> {
    let lines: Vec<&str> = text.split('\n').map(str::trim).collect();
    saved
        .iter()
        .filter_map(|(line, content)| {
            let content = content.trim();
            if content.is_empty() {
                return None;
            }
            let index = line.checked_sub(1)?;
            if lines.get(index) == Some(&content) {
                return Some(*line);
            }
            lines
                .iter()
                .enumerate()
                .filter(|(_, candidate)| **candidate == content)
                .min_by_key(|(candidate, _)| candidate.abs_diff(index))
                .map(|(candidate, _)| candidate + 1)
        })
        .collect()
}

/// The line without a trailing `#` comment (ignoring `#` inside strings).
fn strip_comment(line: &str) -> &str {
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (i, c) in line.char_indices() {
        match quote {
            Some(q) => {
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == q {
                    quote = None;
                }
            }
            None if c == '#' => return &line[..i],
            None if c == '"' || c == '\'' => quote = Some(c),
            None => {}
        }
    }
    line
}

#[cfg(test)]
#[path = "../../tests/rust/editor/test_text_ops.rs"]
mod tests;
