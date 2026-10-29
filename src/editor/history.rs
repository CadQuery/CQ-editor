//! The editor's undo history.
//!
//! egui's `TextEdit` keeps a history of its own: it takes a snapshot once
//! the text and the caret have been still for a second, so an indent or a
//! completion may be undone together with the typing around it, a caret
//! move counts as a step, and the history lives in egui's memory, where it
//! outlives the document (undo after opening another file brought back the
//! previous file's text). The editor records explicit steps instead and
//! never lets `TextEdit` see the undo keys:
//!
//! * typing is one step while it continues at the caret: characters typed
//!   one after another, or deleted one after another, without a pause, a
//!   caret move or a new line;
//! * every editor command (indent, comment, completion, replace) and every
//!   change made outside the editor (a reload from disk) is one step;
//! * adjusting one value, such as dragging a parameter, is one step while
//!   it continues without a pause.

use std::collections::VecDeque;

use super::text_ops::Selection;

/// Steps kept for undo.
const MAX_STEPS: usize = 1000;
/// Text kept by the undo and redo steps together.
const MAX_BYTES: usize = 32 * 1024 * 1024;
/// A pause this long, in seconds, ends a typing step.
const TYPING_PAUSE: f64 = 1.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKind {
    /// Typing in the text field; keystrokes at the caret are grouped.
    Typing,
    /// An editor command or a change from outside the editor.
    Command,
    /// A new value for the literal starting at this byte; successive
    /// values of the same literal are grouped.
    Adjust(usize),
}

/// `removed`, at byte `start`, was replaced by `inserted`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Change {
    start: usize,
    removed: String,
    inserted: String,
}

impl Change {
    fn bytes(&self) -> usize {
        self.removed.len() + self.inserted.len()
    }
}

/// The changes a step groups: typing, or the values of one literal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Keystrokes {
    Insert,
    Delete,
    Adjust(usize),
}

#[derive(Clone, Debug)]
struct Step {
    /// Applied in order.
    changes: Vec<Change>,
    before: Selection,
    after: Selection,
    /// Set while typing may still extend the step.
    keystrokes: Option<Keystrokes>,
    time: f64,
}

impl Step {
    fn bytes(&self) -> usize {
        self.changes.iter().map(Change::bytes).sum()
    }
}

#[derive(Default)]
pub struct History {
    undo: VecDeque<Step>,
    redo: Vec<Step>,
    /// The text the history last saw; `None` until the first record.
    text: Option<String>,
    bytes: usize,
}

impl History {
    /// Forgets everything, for another document.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Records how the text changed since the last record, if it did. The
    /// selection was `before` just before the change and is `after` now.
    pub fn record(
        &mut self,
        text: &str,
        before: &Selection,
        after: &Selection,
        kind: EditKind,
        now: f64,
    ) {
        let Some(previous) = &self.text else {
            self.text = Some(text.to_owned());
            return;
        };
        if previous == text {
            return;
        }
        let change = diff(previous, text);
        self.text = Some(text.to_owned());
        self.bytes -= self.redo.drain(..).map(|step| step.bytes()).sum::<usize>();
        self.bytes += change.bytes();

        let keystrokes = match kind {
            EditKind::Typing => keystrokes(&change),
            EditKind::Command => None,
            EditKind::Adjust(literal) => Some(Keystrokes::Adjust(literal)),
        };
        if let Some(keystrokes) = keystrokes
            && let Some(last) = self.undo.back_mut()
            && last.keystrokes == Some(keystrokes)
            && last.after == *before
            && now - last.time < TYPING_PAUSE
        {
            last.changes.push(change);
            last.after = after.clone();
            last.time = now;
            return;
        }
        self.undo.push_back(Step {
            changes: vec![change],
            before: before.clone(),
            after: after.clone(),
            keystrokes,
            time: now,
        });
        self.trim();
    }

    /// Reverts the last step of `text`, which must be the recorded text.
    /// Returns the text and the selection from before that step.
    pub fn undo(&mut self, text: &str) -> Option<(String, Selection)> {
        if self.text.as_deref() != Some(text) {
            return None;
        }
        let step = self.undo.pop_back()?;
        let mut text = text.to_owned();
        for change in step.changes.iter().rev() {
            text.replace_range(
                change.start..change.start + change.inserted.len(),
                &change.removed,
            );
        }
        let selection = step.before.clone();
        self.redo.push(step);
        self.close_typing();
        self.text = Some(text.clone());
        Some((text, selection))
    }

    /// Applies the last undone step again. Returns the text and the
    /// selection after that step.
    pub fn redo(&mut self, text: &str) -> Option<(String, Selection)> {
        if self.text.as_deref() != Some(text) {
            return None;
        }
        let step = self.redo.pop()?;
        let mut text = text.to_owned();
        for change in &step.changes {
            text.replace_range(
                change.start..change.start + change.removed.len(),
                &change.inserted,
            );
        }
        let selection = step.after.clone();
        self.undo.push_back(step);
        self.close_typing();
        self.text = Some(text.clone());
        Some((text, selection))
    }

    /// Typing after an undo or a redo starts a step of its own.
    fn close_typing(&mut self) {
        if let Some(last) = self.undo.back_mut() {
            last.keystrokes = None;
        }
    }

    fn trim(&mut self) {
        while self.undo.len() > MAX_STEPS || (self.bytes > MAX_BYTES && self.undo.len() > 1) {
            if let Some(step) = self.undo.pop_front() {
                self.bytes -= step.bytes();
            }
        }
    }
}

/// The keystrokes a change could be part of: one character typed (over a
/// selection or not), or one character deleted. A new line ends the
/// typing: it is a step of its own.
fn keystrokes(change: &Change) -> Option<Keystrokes> {
    let single = |text: &str| text.chars().nth(1).is_none() && !text.is_empty();
    if single(&change.inserted) && change.inserted != "\n" {
        Some(Keystrokes::Insert)
    } else if change.inserted.is_empty() && single(&change.removed) {
        Some(Keystrokes::Delete)
    } else {
        None
    }
}

/// The smallest change that turns `old` into `new`.
fn diff(old: &str, new: &str) -> Change {
    let (a, b) = (old.as_bytes(), new.as_bytes());
    let mut prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    while !old.is_char_boundary(prefix) {
        prefix -= 1;
    }
    let limit = a.len().min(b.len()) - prefix;
    let mut suffix = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take(limit)
        .take_while(|(x, y)| x == y)
        .count();
    while !old.is_char_boundary(old.len() - suffix) {
        suffix -= 1;
    }
    Change {
        start: prefix,
        removed: old[prefix..old.len() - suffix].to_owned(),
        inserted: new[prefix..new.len() - suffix].to_owned(),
    }
}

#[cfg(test)]
#[path = "../../tests/rust/editor/test_history.rs"]
mod tests;
