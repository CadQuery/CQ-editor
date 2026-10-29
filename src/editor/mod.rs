//! The code editor panel.
//!
//! Built on egui's `TextEdit` with a custom layouter for syntax highlighting.
//! Around it the editor adds a gutter (line numbers, breakpoints, the debug
//! position), current/error/debug line backgrounds, a line-length ruler, a
//! search and replace bar, a *Go to line* bar, the completion popup, hover
//! documentation and its own undo history.

pub mod highlight;
pub mod history;
pub mod snippets;
pub mod text_ops;

use std::collections::{BTreeMap, BTreeSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;

use egui::text::{CCursor, CCursorRange};
use egui::{
    Align, Align2, Color32, FontId, Galley, Key, Modifiers, Pos2, Rect, Sense, Shape, Stroke,
    TextBuffer, TextEdit, Vec2,
};

use crate::commands::{self, Action};
use crate::document::Document;
use crate::settings::SavedBreakpoint;
use crate::worker::protocol::{CompletionItem, Documentation};
use highlight::SyntaxColors;
use history::{EditKind, History};
use text_ops::Selection;

/// Most search matches highlighted at once.
const MAX_HIGHLIGHTED_MATCHES: usize = 2000;
const COMPLETION_ROWS: usize = 10;
/// Seconds the pointer rests on a name before its documentation is asked for.
const HOVER_DELAY: f64 = 0.6;
/// Width of the documentation tooltip, in points.
const HOVER_WIDTH: f32 = 520.0;

#[derive(Clone, Debug, PartialEq)]
pub struct EditorColors {
    pub syntax: SyntaxColors,
    pub gutter_text: Color32,
    pub gutter_text_current: Color32,
    pub current_line: Color32,
    pub ruler: Color32,
    pub breakpoint: Color32,
    pub debug_line: Color32,
    pub debug_marker: Color32,
    pub error_line: Color32,
    pub error_text: Color32,
    pub search_match: Color32,
    pub search_current: Color32,
}

pub struct EditorOptions<'a> {
    pub font_size: f32,
    pub line_wrap: bool,
    /// Ruler column; 0 hides it.
    pub ruler: usize,
    pub highlight_current_line: bool,
    pub auto_indent: bool,
    /// Offer completions after this many characters of a name; 0 only
    /// after `.` and on request.
    pub completion_min_chars: usize,
    pub colors: &'a EditorColors,
}

/// Where the editor wants completions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompletionRequest {
    /// 1-based.
    pub line: usize,
    /// 0-based, in characters.
    pub column: usize,
    /// Offered while typing a name, not asked for: an empty answer is
    /// not worth a message.
    pub automatic: bool,
}

/// Where the editor wants the documentation of the name under the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HoverRequest {
    /// 1-based.
    pub line: usize,
    /// 0-based, in characters.
    pub column: usize,
}

#[derive(Default)]
pub struct EditorOutput {
    pub changed: bool,
    pub completion_request: Option<CompletionRequest>,
    pub hover_request: Option<HoverRequest>,
}

#[derive(Default)]
struct Search {
    open: bool,
    query: String,
    match_case: bool,
    whole_word: bool,
    matches: Vec<Range<usize>>,
    current: usize,
    focus_query: bool,
    computed_for: u64,
    /// The match the bar selected last, while the text and the query are
    /// those of `computed_for`. An unfocused `TextEdit` collapses its
    /// selection to the caret, so while the replace field has the keyboard
    /// the editor's selection cannot tell which match is selected.
    visited: Option<(Range<usize>, u64)>,
    /// The replace row is shown.
    replacing: bool,
    replacement: String,
    focus_replacement: bool,
}

/// What the replace row was asked to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Replace {
    Current,
    All,
}

/// The *Go to line* bar.
#[derive(Default)]
struct GoToLine {
    input: String,
    focus: bool,
}

/// Documentation of the name under the pointer.
#[derive(Default)]
struct Hover {
    /// The name under the pointer and when the pointer arrived on it.
    target: Option<(Range<usize>, f64)>,
    /// The name whose documentation was asked for.
    requested: Option<Range<usize>>,
    /// Documentation of the `requested` name, once it arrived.
    text: Option<Documentation>,
}

#[derive(Default)]
struct Completion {
    items: Vec<CompletionItem>,
    selected: usize,
    /// Cursor position (byte) when the completion was requested.
    anchor: usize,
    open: bool,
    scroll_to_selected: bool,
}

/// The highlighted layout job of the last text. Galleys themselves are not
/// kept: egui rebuilds its font atlas now and then, which invalidates old
/// galleys, while its own galley cache handles that.
#[derive(Default)]
struct LayoutCache {
    key: u64,
    job: Option<egui::text::LayoutJob>,
    /// Width of the last laid out text, for horizontal scrolling.
    width: f32,
}

/// What a profiled run spent on one line (FEAT-E2).
#[derive(Clone, Debug, PartialEq)]
pub struct LineTiming {
    /// With everything the line called.
    pub seconds: f64,
    pub hits: u64,
    /// Its slowest calls into CadQuery: name, seconds and calls.
    pub operations: Vec<(String, f64, u64)>,
}

/// Lines faster than this get no time in the gutter.
const PROFILE_FLOOR_SECONDS: f64 = 0.001;
/// Characters of the gutter's time column ("999 ms", "12.3 s").
const PROFILE_CHARS: f32 = 6.0;

/// A duration as the gutter shows it: `850 ms`, `1.2 s`, `14 s`.
pub fn format_seconds(seconds: f64) -> String {
    if seconds >= 10.0 {
        format!("{seconds:.0} s")
    } else if seconds >= 0.9995 {
        format!("{seconds:.1} s")
    } else if seconds >= PROFILE_FLOOR_SECONDS {
        format!("{:.0} ms", seconds * 1000.0)
    } else {
        "<1 ms".to_owned()
    }
}

pub struct CodeEditor {
    /// 1-based line numbers.
    pub breakpoints: BTreeSet<usize>,
    /// The last profiled run's time per 1-based line; it follows edits like
    /// the breakpoints do.
    pub line_profile: BTreeMap<usize, LineTiming>,
    pub debug_line: Option<usize>,
    pub error_line: Option<usize>,
    /// The text the line markers above refer to; see [`Self::sync_markers`].
    marked_text: Option<String>,
    text_id: egui::Id,
    /// Last known selection, in bytes.
    selection: Selection,
    pending_selection: Option<Selection>,
    pending_scroll_line: Option<usize>,
    pending_focus: bool,
    completion_wanted: bool,
    /// Cursor position (byte) of the last completion request; the worker's
    /// answer completes the text from there.
    completion_anchor: usize,
    completion: Completion,
    calltip: Option<String>,
    search: Search,
    go_to: Option<GoToLine>,
    hover: Hover,
    history: History,
    /// Time of the current frame, in seconds; groups typing into steps.
    now: f64,
    layout: LayoutCache,
}

impl Default for CodeEditor {
    fn default() -> Self {
        Self::with_id(egui::Id::new("code_editor_text"))
    }
}

impl CodeEditor {
    /// An editor whose widgets keep their state (caret, scrolling) in egui
    /// under `id`: each tab has its own, so switching tabs keeps them.
    pub fn with_id(id: egui::Id) -> Self {
        Self {
            breakpoints: BTreeSet::new(),
            line_profile: BTreeMap::new(),
            debug_line: None,
            error_line: None,
            marked_text: None,
            text_id: id,
            selection: Selection::caret(0),
            pending_selection: None,
            pending_scroll_line: None,
            pending_focus: false,
            completion_wanted: false,
            completion_anchor: 0,
            completion: Completion::default(),
            calltip: None,
            search: Search::default(),
            go_to: None,
            hover: Hover::default(),
            history: History::default(),
            now: 0.0,
            layout: LayoutCache::default(),
        }
    }

    /// 1-based line and column of the cursor, for the status bar.
    pub fn cursor_position(&self, text: &str) -> (usize, usize) {
        let (line, column) = text_ops::line_column(text, self.selection.cursor.min(text.len()));
        (line + 1, column + 1)
    }

    /// Forgets per-document state when another file is loaded.
    pub fn reset(&mut self) {
        self.breakpoints.clear();
        self.line_profile.clear();
        self.debug_line = None;
        self.error_line = None;
        self.marked_text = None;
        self.close_popups();
        self.hover = Hover::default();
        self.history.clear();
        self.pending_selection = Some(Selection::caret(0));
        self.pending_scroll_line = Some(1);
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// Reverts the last edit step (typing, a command or a reload).
    pub fn undo(&mut self, document: &mut Document) {
        self.record_outside_edits(document);
        if let Some((text, selection)) = self.history.undo(&document.text) {
            self.restore(document, text, selection);
        }
    }

    pub fn redo(&mut self, document: &mut Document) {
        self.record_outside_edits(document);
        if let Some((text, selection)) = self.history.redo(&document.text) {
            self.restore(document, text, selection);
        }
    }

    fn restore(&mut self, document: &mut Document, text: String, selection: Selection) {
        document.text = text;
        if self.marked_text.is_some() {
            self.sync_markers(&document.text);
        }
        self.close_popups();
        self.selection = selection.clone();
        self.pending_scroll_line = Some(text_ops::line_of(&document.text, selection.cursor) + 1);
        self.pending_selection = Some(selection);
        self.pending_focus = true;
    }

    /// Makes a change made outside the editor, such as a reload from disk,
    /// a step of its own.
    fn record_outside_edits(&mut self, document: &Document) {
        let selection = self.clamped_selection(&document.text);
        self.history.record(
            &document.text,
            &selection,
            &selection,
            EditKind::Command,
            self.now,
        );
    }

    /// The breakpoints with the text of their lines, to find them again
    /// in a later session even if lines were added above them meanwhile.
    pub fn saved_breakpoints(&mut self, text: &str) -> Vec<SavedBreakpoint> {
        self.sync_markers(text);
        self.breakpoints
            .iter()
            .map(|&line| SavedBreakpoint {
                line,
                text: text[text_ops::line_range(text, line - 1)].trim().to_owned(),
            })
            .collect()
    }

    /// Restores breakpoints of an earlier session; see [`text_ops::relocate_lines`].
    pub fn restore_breakpoints(&mut self, text: &str, saved: &[SavedBreakpoint]) {
        let saved: Vec<(usize, String)> = saved
            .iter()
            .map(|breakpoint| (breakpoint.line, breakpoint.text.clone()))
            .collect();
        self.breakpoints = text_ops::relocate_lines(text, &saved);
        self.marked_text = Some(text.to_owned());
    }

    /// Gives the keyboard to the text on the next frame.
    pub fn focus(&mut self) {
        self.pending_focus = true;
    }

    pub fn go_to_line(&mut self, text: &str, line: usize) {
        let start = text_ops::line_start(text, line.saturating_sub(1));
        self.pending_selection = Some(Selection::caret(start));
        self.pending_scroll_line = Some(line);
        self.pending_focus = true;
    }

    pub fn set_debug_line(&mut self, line: Option<usize>) {
        self.debug_line = line;
        if line.is_some() {
            self.pending_scroll_line = line;
        }
    }

    /// Moves breakpoints and the error and debug lines along with the edits
    /// made since the last call, so they stay on their statements when lines
    /// are added or removed above them. Edits come from typing, the editor
    /// commands and reloads alike, so this compares texts instead of hooking
    /// each of them.
    pub fn sync_markers(&mut self, text: &str) {
        if let Some(marked) = &self.marked_text {
            if marked == text {
                return;
            }
            let map = text_ops::LineMap::new(marked, text);
            self.breakpoints = self
                .breakpoints
                .iter()
                .filter_map(|&line| map.map(line))
                .collect();
            self.error_line = self.error_line.and_then(|line| map.map(line));
            self.debug_line = self.debug_line.and_then(|line| map.map(line));
            self.line_profile = std::mem::take(&mut self.line_profile)
                .into_iter()
                .filter_map(|(line, timing)| Some((map.map(line)?, timing)))
                .collect();
        }
        self.marked_text = Some(text.to_owned());
    }

    /// Shows the times of a profiled run of `run_text` in the gutter of
    /// `text`, the document as it is now (edited, perhaps, while it ran).
    pub fn set_line_profile(
        &mut self,
        profile: BTreeMap<usize, LineTiming>,
        run_text: &str,
        text: &str,
    ) {
        self.sync_markers(text);
        let map = text_ops::LineMap::new(run_text, text);
        self.line_profile = profile
            .into_iter()
            .filter_map(|(line, timing)| Some((map.map(line)?, timing)))
            .collect();
    }

    pub fn toggle_breakpoint(&mut self, line: usize) {
        if !self.breakpoints.remove(&line) {
            self.breakpoints.insert(line);
        }
    }

    pub fn toggle_comment(&mut self, document: &mut Document) {
        let edited =
            text_ops::toggle_comment(&document.text, &self.clamped_selection(&document.text));
        self.apply(document, edited);
    }

    pub fn duplicate_line(&mut self, document: &mut Document) {
        let edited =
            text_ops::duplicate_lines(&document.text, &self.clamped_selection(&document.text));
        self.pending_scroll_line =
            Some(text_ops::line_of(&edited.text, edited.selection.cursor) + 1);
        self.apply(document, edited);
    }

    pub fn move_lines(&mut self, document: &mut Document, up: bool) {
        self.sync_markers(&document.text);
        let selection = self.clamped_selection(&document.text);
        let lines = text_ops::selected_lines(&document.text, &selection);
        let total_lines = document.text.matches('\n').count() + 1;
        let edited = if up {
            if lines.start == 0 {
                return;
            }
            text_ops::move_lines_up(&document.text, &selection)
        } else {
            if lines.end >= total_lines {
                return;
            }
            text_ops::move_lines_down(&document.text, &selection)
        };
        let first_1 = lines.start + 1;
        let last_1 = lines.end;
        let remap = |line: usize| -> usize {
            if up {
                if line == first_1 - 1 {
                    last_1
                } else if (first_1..=last_1).contains(&line) {
                    line - 1
                } else {
                    line
                }
            } else if line == last_1 + 1 {
                first_1
            } else if (first_1..=last_1).contains(&line) {
                line + 1
            } else {
                line
            }
        };
        self.breakpoints = self.breakpoints.iter().map(|&l| remap(l)).collect();
        self.error_line = self.error_line.map(remap);
        self.debug_line = self.debug_line.map(remap);
        self.line_profile = std::mem::take(&mut self.line_profile)
            .into_iter()
            .map(|(line, timing)| (remap(line), timing))
            .collect();
        self.marked_text = Some(edited.text.clone());
        self.pending_scroll_line =
            Some(text_ops::line_of(&edited.text, edited.selection.cursor) + 1);
        self.apply(document, edited);
    }

    pub fn insert_snippet(&mut self, document: &mut Document, snippet: &snippets::Snippet) {
        let selection = self.clamped_selection(&document.text);
        let edited = text_ops::insert_snippet(
            &document.text,
            &selection,
            snippet.code,
            snippet.needs_import,
        );
        self.pending_scroll_line =
            Some(text_ops::line_of(&edited.text, edited.selection.cursor) + 1);
        self.apply(document, edited);
    }

    /// Replaces `range` with `replacement` as a new value of the literal
    /// there: successive values, such as those of a drag in the Parameters
    /// panel, are one undo step. The caret stays on the text it was on and
    /// the keyboard focus stays where it is. `now` is in seconds.
    pub fn adjust_text(
        &mut self,
        document: &mut Document,
        range: Range<usize>,
        replacement: &str,
        now: f64,
    ) {
        if document.text.get(range.clone()) == Some(replacement) {
            return;
        }
        self.record_outside_edits(document);
        let before = self.clamped_selection(&document.text);
        let shift = |at: usize| {
            if at >= range.end {
                at - range.end + range.start + replacement.len()
            } else {
                at.min(range.start + replacement.len())
            }
        };
        let after = Selection {
            anchor: shift(before.anchor),
            cursor: shift(before.cursor),
        };
        document.text.replace_range(range.clone(), replacement);
        self.history.record(
            &document.text,
            &before,
            &after,
            EditKind::Adjust(range.start),
            now,
        );
        self.close_popups();
        self.selection = after.clone();
        self.pending_selection = Some(after);
    }

    /// Replaces multiple non-overlapping ranges (sorted by start byte) with
    /// their replacements in a single undoable command step.
    pub fn replace_literals(
        &mut self,
        document: &mut Document,
        replacements: &[(Range<usize>, String)],
        now: f64,
    ) {
        if replacements.is_empty() {
            return;
        }
        self.record_outside_edits(document);
        let before = self.clamped_selection(&document.text);
        let mut after = before.clone();
        for (range, replacement) in replacements.iter().rev() {
            if document.text.get(range.clone()) == Some(replacement.as_str()) {
                continue;
            }
            let shift = |at: usize| {
                if at >= range.end {
                    at - range.end + range.start + replacement.len()
                } else {
                    at.min(range.start + replacement.len())
                }
            };
            after.anchor = shift(after.anchor);
            after.cursor = shift(after.cursor);
            document.text.replace_range(range.clone(), replacement);
        }
        self.history
            .record(&document.text, &before, &after, EditKind::Command, now);
        self.close_popups();
        self.selection = after.clone();
        self.pending_selection = Some(after);
    }

    /// Replaces the selection with `text` as one undo step and leaves the
    /// caret after it, e.g. to insert a selector picked in the 3D view.
    pub fn insert_text(&mut self, document: &mut Document, text: &str) {
        let range = self.clamped_selection(&document.text).range();
        let mut edited = document.text.clone();
        edited.replace_range(range.clone(), text);
        let caret = range.start + text.len();
        self.pending_scroll_line = Some(text_ops::line_of(&edited, caret) + 1);
        self.apply(
            document,
            text_ops::Edited {
                text: edited,
                selection: Selection::caret(caret),
            },
        );
    }

    pub fn open_search(&mut self, document: &Document) {
        let range = self.clamped_selection(&document.text).range();
        let selected = &document.text[range];
        if !selected.is_empty() && !selected.contains('\n') {
            self.search.query = selected.to_owned();
        }
        self.search.open = true;
        self.search.focus_query = true;
        self.search.computed_for = 0;
        self.go_to = None;
    }

    /// Opens the search bar with its replace row.
    pub fn open_replace(&mut self, document: &Document) {
        let had_query = !self.search.query.is_empty();
        self.open_search(document);
        self.search.replacing = true;
        // With the query already filled in, the replacement is next.
        if had_query || !self.search.query.is_empty() {
            self.search.focus_query = false;
            self.search.focus_replacement = true;
        }
    }

    pub fn open_go_to_line(&mut self) {
        self.go_to = Some(GoToLine {
            input: String::new(),
            focus: true,
        });
        self.search.open = false;
    }

    /// Documentation for the name the editor asked about; empty when there
    /// is none.
    pub fn show_documentation(&mut self, documentation: Documentation) {
        let current = self.hover.target.as_ref().map(|(range, _)| range);
        if current.is_some() && current == self.hover.requested.as_ref() {
            self.hover.text = Some(documentation);
        }
    }

    pub fn search_is_open(&self) -> bool {
        self.search.open
    }

    /// Selects the match after the selection (or before it), wrapping
    /// around: from where the caret is, not from the last match visited.
    pub fn find_next(&mut self, document: &Document, forward: bool) {
        self.update_matches(&document.text);
        let selection = self.clamped_selection(&document.text).range();
        if let Some(index) = next_match(&self.search.matches, &selection, forward) {
            self.search.current = index;
            self.select_current_match(&document.text);
        }
    }

    /// Asks for completions at the cursor on the next frame.
    pub fn request_completion(&mut self) {
        self.completion_wanted = true;
    }

    /// Completion results from the worker. Returns `false` when there is
    /// nothing to show.
    pub fn show_completions(
        &mut self,
        items: Vec<CompletionItem>,
        signatures: Vec<String>,
    ) -> bool {
        self.calltip = None;
        self.completion.open = false;
        if !items.is_empty() {
            self.completion = Completion {
                items,
                selected: 0,
                anchor: self.completion_anchor,
                open: true,
                scroll_to_selected: true,
            };
            true
        } else if !signatures.is_empty() {
            self.calltip = Some(signatures.join("\n"));
            true
        } else {
            false
        }
    }

    pub fn close_popups(&mut self) {
        self.completion.open = false;
        self.calltip = None;
    }

    fn clamped_selection(&self, text: &str) -> Selection {
        let clamp = |i: usize| floor_char_boundary(text, i.min(text.len()));
        Selection {
            anchor: clamp(self.selection.anchor),
            cursor: clamp(self.selection.cursor),
        }
    }

    /// Applies an editor command as one undo step.
    fn apply(&mut self, document: &mut Document, edited: text_ops::Edited) {
        if edited.text != document.text {
            self.record_outside_edits(document);
            if self.marked_text.is_some() {
                self.sync_markers(&document.text);
            }
            let before = self.clamped_selection(&document.text);
            document.text = edited.text;
            if self.marked_text.is_some() {
                self.sync_markers(&document.text);
            }
            self.history.record(
                &document.text,
                &before,
                &edited.selection,
                EditKind::Command,
                self.now,
            );
        }
        self.selection = edited.selection.clone();
        self.pending_selection = Some(edited.selection);
        self.pending_focus = true;
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        document: &mut Document,
        options: &EditorOptions<'_>,
    ) -> EditorOutput {
        let mut output = EditorOutput::default();
        self.now = ui.input(|i| i.time);
        self.selection = self.read_selection(ui.ctx(), &document.text);
        // A reload since the last frame is an undo step of its own.
        self.record_outside_edits(document);
        if self.search.open {
            self.search_bar(ui, document);
        }
        if self.go_to.is_some() {
            self.go_to_bar(ui, document);
        }

        let font = FontId::monospace(options.font_size);
        let (row_height, char_width) =
            ui.fonts_mut(|f| (f.row_height(&font), f.glyph_width(&font, 'M')));

        // Keys the editor handles itself must be taken before TextEdit sees
        // them; that includes undo and redo, so `TextEdit`'s own history,
        // which does not know the editor commands, is never used.
        if ui.memory(|m| m.has_focus(self.text_id)) {
            output.changed |= self.handle_keys(ui, document, options);
        }
        if let Some(selection) = self.pending_selection.take() {
            self.write_selection(ui.ctx(), &document.text, &selection);
            self.selection = selection;
        }
        let before_typing = self.clamped_selection(&document.text);
        if std::mem::take(&mut self.pending_focus) {
            ui.memory_mut(|m| m.request_focus(self.text_id));
        }

        let line_count = document.text.matches('\n').count() + 1;
        let digits = line_count.to_string().len().max(2);
        let number_width = digits as f32 * char_width;
        let profile_width = if self.line_profile.is_empty() {
            0.0
        } else {
            PROFILE_CHARS * char_width + 8.0
        };
        let gutter_width = number_width + 28.0 + profile_width;

        let dark = ui.visuals().dark_mode;
        let colors = options.colors;
        let area = ui.available_rect_before_wrap();
        let gutter_rect = Rect::from_min_size(area.min, Vec2::new(gutter_width, area.height()));
        let text_rect = Rect::from_min_max(Pos2::new(gutter_rect.right(), area.top()), area.max);
        ui.allocate_rect(area, Sense::hover());

        // The text scrolls both ways; the gutter only follows it vertically,
        // so the line numbers stay in view.
        let mut text_ui = ui.new_child(egui::UiBuilder::new().max_rect(text_rect));
        let geometry = egui::ScrollArea::both()
            .id_salt(self.text_id.with("scroll"))
            .auto_shrink([false, false])
            .show(&mut text_ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                // Placeholder for backgrounds that must sit under the text.
                let background = ui.painter().add(Shape::Noop);

                let wrap = options.line_wrap;
                let previous_width = self.layout.width;
                let cache = &mut self.layout;
                let mut layouter = |ui: &egui::Ui, buffer: &dyn TextBuffer, wrap_width: f32| {
                    let text = buffer.as_str();
                    let wrap_width = if wrap { wrap_width } else { f32::INFINITY };
                    let key = layout_key(text, wrap_width, font.size, dark);
                    if cache.key != key || cache.job.is_none() {
                        cache.key = key;
                        cache.job = Some(highlight::layout_job(
                            text,
                            font.clone(),
                            &colors.syntax,
                            wrap_width,
                        ));
                    }
                    let job = cache.job.clone().unwrap_or_default();
                    let galley = ui.fonts_mut(|f| f.layout_job(job));
                    cache.width = galley.size().x;
                    galley
                };

                let available = ui.available_size();
                // Without wrapping the text edit must be as wide as its
                // longest line, so the scroll area can scroll sideways
                // (last frame's layout is a good enough measure).
                let text_width = if wrap {
                    available.x
                } else {
                    (previous_width + 24.0).max(available.x)
                };
                let edit = TextEdit::multiline(&mut document.text)
                    .id(self.text_id)
                    .font(font.clone())
                    .lock_focus(true)
                    .frame(egui::Frame::NONE)
                    .margin(egui::Margin::symmetric(6, 2))
                    .desired_width(if wrap { available.x } else { f32::INFINITY })
                    .min_size(Vec2::new(text_width, available.y.max(row_height)))
                    .layouter(&mut layouter)
                    .show(ui);
                let text_changed = edit.response.response.changed();
                output.changed |= text_changed;
                if let Some(range) = edit.cursor_range {
                    self.selection = Selection {
                        anchor: text_ops::char_to_byte(&document.text, range.secondary.index.0),
                        cursor: text_ops::char_to_byte(&document.text, range.primary.index.0),
                    };
                }
                if text_changed {
                    self.history.record(
                        &document.text,
                        &before_typing,
                        &self.selection,
                        EditKind::Typing,
                        self.now,
                    );
                    self.close_popups_if_left(&document.text);
                    self.hover = Hover::default();
                }
                // Before the line backgrounds and the gutter are drawn.
                self.sync_markers(&document.text);

                // Typing "." asks for member completions, and typing the
                // first characters of a name for names.
                let typed = |test: fn(&str) -> bool| {
                    text_changed
                        && ui.input(|i| {
                            i.events.iter().any(
                                |e| matches!(e, egui::Event::Text(t) if !t.is_empty() && test(t)),
                            )
                        })
                };
                let typed_dot = typed(|t| t == ".");
                let typed_name = !self.completion.open
                    && typed(|t| t.chars().all(text_ops::is_word_char))
                    && completes_name(
                        &document.text,
                        self.selection.cursor,
                        options.completion_min_chars,
                    );
                let wanted = std::mem::take(&mut self.completion_wanted);
                if typed_dot || typed_name || wanted {
                    let cursor = self.selection.cursor.min(document.text.len());
                    let (line, column) = text_ops::line_column(&document.text, cursor);
                    self.completion_anchor = cursor;
                    output.completion_request = Some(CompletionRequest {
                        line: line + 1,
                        column,
                        automatic: typed_name && !typed_dot && !wanted,
                    });
                }

                output.hover_request = self.hover(ui, &edit, &document.text);

                let geometry = Geometry::new(&edit.galley, edit.galley_pos);
                let text_left = edit.response.response.rect.left();
                let shapes = self.backgrounds(
                    &geometry,
                    &document.text,
                    options,
                    text_left,
                    ui.clip_rect(),
                    char_width,
                );
                ui.painter().set(background, Shape::Vec(shapes));

                if let Some(line) = self.pending_scroll_line.take()
                    && let Some(rect) = geometry.line_rect(line.saturating_sub(1))
                {
                    // Aim at the start of the line: vertical scrolling only.
                    let target = Rect::from_min_size(
                        Pos2::new(text_left, rect.top()),
                        Vec2::new(1.0, rect.height()),
                    );
                    ui.scroll_to_rect(target, Some(Align::Center));
                }

                let caret = geometry.caret_rect(&document.text, self.selection.cursor);
                self.popups(ui, document, caret, row_height);
                geometry
            })
            .inner;

        self.paint_gutter(ui, &geometry, gutter_rect, colors, number_width);
        let gutter = ui.interact(gutter_rect, self.text_id.with("gutter"), Sense::click());
        if gutter.clicked()
            && let Some(line) = gutter
                .interact_pointer_pos()
                .and_then(|p| geometry.line_at(p.y))
        {
            self.toggle_breakpoint(line + 1);
        }
        gutter.on_hover_cursor(egui::CursorIcon::PointingHand);
        output
    }

    fn read_selection(&self, ctx: &egui::Context, text: &str) -> Selection {
        TextEdit::load_state(ctx, self.text_id)
            .and_then(|state| state.cursor.char_range())
            .map_or(self.selection.clone(), |range| Selection {
                anchor: text_ops::char_to_byte(text, range.secondary.index.0),
                cursor: text_ops::char_to_byte(text, range.primary.index.0),
            })
    }

    fn write_selection(&self, ctx: &egui::Context, text: &str, selection: &Selection) {
        let mut state = TextEdit::load_state(ctx, self.text_id).unwrap_or_default();
        state.cursor.set_char_range(Some(CCursorRange {
            primary: CCursor::new(text_ops::byte_to_char(text, selection.cursor)),
            secondary: CCursor::new(text_ops::byte_to_char(text, selection.anchor)),
            h_pos: None,
        }));
        TextEdit::store_state(ctx, self.text_id, state);
    }

    /// Handles Tab, Shift+Tab, Enter and the completion popup's keys.
    fn handle_keys(
        &mut self,
        ui: &egui::Ui,
        document: &mut Document,
        options: &EditorOptions<'_>,
    ) -> bool {
        let consume = |modifiers, key| ui.input_mut(|i| i.consume_key(modifiers, key));
        // egui ignores an extra Shift when matching, so Ctrl+Shift+Z first.
        if consume(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z)
            || consume(Modifiers::COMMAND, Key::Y)
        {
            self.redo(document);
            return true;
        }
        if consume(Modifiers::COMMAND, Key::Z) {
            self.undo(document);
            return true;
        }
        let selection = self.clamped_selection(&document.text);

        if self.completion.open {
            let visible = self.visible_completions(&document.text);
            if consume(Modifiers::NONE, Key::Escape) {
                self.completion.open = false;
            } else if consume(Modifiers::NONE, Key::ArrowDown) {
                self.completion.selected =
                    (self.completion.selected + 1).min(visible.len().saturating_sub(1));
                self.completion.scroll_to_selected = true;
            } else if consume(Modifiers::NONE, Key::ArrowUp) {
                self.completion.selected = self.completion.selected.saturating_sub(1);
                self.completion.scroll_to_selected = true;
            } else if !visible.is_empty()
                && (consume(Modifiers::NONE, Key::Enter) || consume(Modifiers::NONE, Key::Tab))
            {
                let index = visible[self.completion.selected.min(visible.len() - 1)];
                self.accept_completion(document, index);
                return true;
            }
        }
        if self.calltip.is_some() && consume(Modifiers::NONE, Key::Escape) {
            self.calltip = None;
        }

        let edited = if consume(Modifiers::SHIFT, Key::Tab) {
            Some(text_ops::unindent(&document.text, &selection))
        } else if consume(Modifiers::NONE, Key::Tab) {
            Some(text_ops::indent(&document.text, &selection))
        } else if options.auto_indent && consume(Modifiers::NONE, Key::Enter) {
            let range = selection.range();
            let insert = text_ops::newline_with_indent(&document.text, range.start);
            let mut text = document.text.clone();
            text.replace_range(range.clone(), &insert);
            Some(text_ops::Edited {
                text,
                selection: Selection::caret(range.start + insert.len()),
            })
        } else {
            None
        };
        match edited {
            Some(edited) => {
                self.apply(document, edited);
                true
            }
            None => false,
        }
    }

    /// Indices of the completion items matching what was typed since the
    /// completion was requested.
    fn visible_completions(&self, text: &str) -> Vec<usize> {
        let typed = typed_since(text, self.completion.anchor, self.selection.cursor);
        self.completion
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.insert.starts_with(typed))
            .map(|(i, _)| i)
            .collect()
    }

    fn accept_completion(&mut self, document: &mut Document, index: usize) {
        let Some(item) = self.completion.items.get(index) else {
            return;
        };
        let cursor = self.selection.cursor.min(document.text.len());
        let typed = typed_since(&document.text, self.completion.anchor, cursor).len();
        let insert = item.insert.get(typed..).unwrap_or_default().to_owned();
        let mut text = document.text.clone();
        text.insert_str(cursor, &insert);
        self.completion.open = false;
        self.apply(
            document,
            text_ops::Edited {
                text,
                selection: Selection::caret(cursor + insert.len()),
            },
        );
    }

    /// Typing outside the identifier being completed closes the popups.
    fn close_popups_if_left(&mut self, text: &str) {
        if !self.completion.open {
            return;
        }
        let typed = typed_text(text, self.completion.anchor, self.selection.cursor);
        if typed.is_none_or(|t| !t.chars().all(text_ops::is_word_char)) {
            self.completion.open = false;
        }
    }

    /// Follows the name under the pointer. Once the pointer has rested on
    /// it, asks for its documentation; once that arrived, shows it.
    fn hover(
        &mut self,
        ui: &egui::Ui,
        edit: &egui::text_edit::TextEditOutput,
        text: &str,
    ) -> Option<HoverRequest> {
        let pointer = edit
            .response
            .response
            .hover_pos()
            .filter(|_| !self.completion.open && self.calltip.is_none());
        let name = pointer.and_then(|pos| name_at(&edit.galley, edit.galley_pos, text, pos));
        if self.hover.target.as_ref().map(|(range, _)| range) != name.as_ref() {
            self.hover = Hover {
                target: name.map(|range| (range, self.now)),
                ..Hover::default()
            };
        }
        let (range, since) = self.hover.target.clone()?;

        if self.hover.requested.as_ref() != Some(&range) {
            let waited = self.now - since;
            if waited < HOVER_DELAY {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_secs_f64(
                        HOVER_DELAY - waited,
                    ));
                return None;
            }
            self.hover.requested = Some(range.clone());
            let (line, column) = text_ops::line_column(text, range.start);
            return Some(HoverRequest {
                line: line + 1,
                column,
            });
        }

        if let (Some(pos), Some(documentation)) = (pointer, &self.hover.text)
            && !(documentation.signatures.is_empty() && documentation.text.is_empty())
        {
            // Below the pointer, or above it in the lower half of the
            // screen, so it never covers the name it describes.
            let below = pos.y < ui.ctx().content_rect().center().y;
            let (pivot, offset) = if below {
                (Align2::LEFT_TOP, Vec2::new(12.0, 18.0))
            } else {
                (Align2::LEFT_BOTTOM, Vec2::new(12.0, -12.0))
            };
            egui::Area::new(self.text_id.with("documentation"))
                .order(egui::Order::Tooltip)
                .pivot(pivot)
                .fixed_pos(pos + offset)
                .interactable(false)
                .constrain(true)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.set_max_width(HOVER_WIDTH);
                        for signature in &documentation.signatures {
                            ui.label(egui::RichText::new(signature).monospace().strong());
                        }
                        if !documentation.text.is_empty() {
                            if !documentation.signatures.is_empty() {
                                ui.separator();
                            }
                            ui.label(&documentation.text);
                        }
                    });
                });
        }
        None
    }

    fn search_bar(&mut self, ui: &mut egui::Ui, document: &mut Document) {
        use egui_phosphor::regular as icon;

        self.update_matches(&document.text);
        let mut navigate: Option<bool> = None;
        let mut close = false;
        let mut replace: Option<Replace> = None;
        let shortcut_hint = |label: &str, action: Action| {
            commands::shortcut(&action).map_or_else(
                || label.to_owned(),
                |shortcut| format!("{label} ({})", ui.ctx().format_shortcut(&shortcut)),
            )
        };
        let previous_hint = shortcut_hint("Previous match", Action::FindPrevious);
        let next_hint = shortcut_hint("Next match", Action::FindNext);
        let replace_hint = shortcut_hint("Replace", Action::Replace);
        let field_width = bar_field_width(ui);
        ui.horizontal(|ui| {
            ui.label(egui_phosphor::regular::MAGNIFYING_GLASS);
            let response = ui.add(
                TextEdit::singleline(&mut self.search.query)
                    .hint_text("Search")
                    .desired_width(field_width),
            );
            if std::mem::take(&mut self.search.focus_query) {
                response.request_focus();
            }
            if response.changed() {
                self.search.computed_for = 0;
                self.search.current = 0;
            }
            if response.has_focus() || response.lost_focus() {
                let (enter, shift, escape) = take_bar_keys(ui);
                if enter {
                    navigate = Some(!shift);
                    response.request_focus();
                }
                close |= escape;
            }
            if ui
                .add_enabled(
                    !self.search.matches.is_empty(),
                    egui::Button::new(egui_phosphor::regular::ARROW_UP),
                )
                .on_hover_text(previous_hint)
                .clicked()
            {
                navigate = Some(false);
            }
            if ui
                .add_enabled(
                    !self.search.matches.is_empty(),
                    egui::Button::new(egui_phosphor::regular::ARROW_DOWN),
                )
                .on_hover_text(next_hint)
                .clicked()
            {
                navigate = Some(true);
            }
            if ui
                .selectable_label(self.search.match_case, "Aa")
                .on_hover_text("Match case")
                .clicked()
            {
                self.search.match_case = !self.search.match_case;
                self.search.computed_for = 0;
            }
            if ui
                .selectable_label(
                    self.search.whole_word,
                    egui::RichText::new("ab").underline(),
                )
                .on_hover_text("Whole word")
                .clicked()
            {
                self.search.whole_word = !self.search.whole_word;
                self.search.computed_for = 0;
            }
            let count = self.search.matches.len();
            ui.weak(match count {
                0 if self.search.query.is_empty() => String::new(),
                0 => "No matches".to_owned(),
                _ => format!("{} of {count}", self.search.current.min(count - 1) + 1),
            });
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if ui.button(icon::X).on_hover_text("Close (Esc)").clicked() {
                    close = true;
                }
                if ui
                    .selectable_label(self.search.replacing, icon::SWAP)
                    .on_hover_text(replace_hint)
                    .clicked()
                {
                    self.search.replacing = !self.search.replacing;
                    self.search.focus_replacement = self.search.replacing;
                }
            });
        });
        if self.search.replacing {
            ui.horizontal(|ui| {
                ui.label(icon::SWAP);
                let response = ui.add(
                    TextEdit::singleline(&mut self.search.replacement)
                        .hint_text("Replace with")
                        .desired_width(field_width),
                );
                if std::mem::take(&mut self.search.focus_replacement) {
                    response.request_focus();
                }
                if response.has_focus() || response.lost_focus() {
                    let (enter, _, escape) = take_bar_keys(ui);
                    if enter {
                        replace = Some(Replace::Current);
                        response.request_focus();
                    }
                    close |= escape;
                }
                let found = !self.search.matches.is_empty();
                if ui
                    .add_enabled(found, egui::Button::new("Replace"))
                    .on_hover_text("Replace this match and go to the next one (Enter)")
                    .clicked()
                {
                    replace = Some(Replace::Current);
                }
                if ui
                    .add_enabled(found, egui::Button::new("Replace all"))
                    .on_hover_text("Replace every match, as one step to undo")
                    .clicked()
                {
                    replace = Some(Replace::All);
                }
            });
        }
        ui.separator();

        match replace {
            Some(Replace::Current) => self.replace_current(document),
            Some(Replace::All) => self.replace_all(document),
            None => {}
        }
        if let Some(forward) = navigate {
            self.find_next(document, forward);
        }
        if close {
            self.search.open = false;
            self.pending_focus = true;
        }
    }

    /// Replaces the selected match and moves on to the next one. Without a
    /// selected match, selects the next one first, so nothing is replaced
    /// unseen.
    fn replace_current(&mut self, document: &mut Document) {
        self.update_matches(&document.text);
        let selected = self.clamped_selection(&document.text).range();
        let visited = self
            .search
            .visited
            .clone()
            .filter(|(_, key)| *key == self.search.computed_for)
            .map(|(range, _)| range);
        let index = self
            .search
            .matches
            .iter()
            .position(|m| *m == selected || Some(m) == visited.as_ref());
        let Some(index) = index else {
            self.find_next(document, true);
            return;
        };
        let range = self.search.matches[index].clone();
        let replacement = self.search.replacement.clone();
        let edited = text_ops::replace_ranges(
            &document.text,
            &Selection::caret(range.start),
            std::slice::from_ref(&range),
            &replacement,
        );
        self.apply(document, edited);
        // The replace field keeps the keyboard for the next Enter.
        self.pending_focus = false;
        self.update_matches(&document.text);
        let after = range.start + replacement.len();
        let next = self.search.matches.iter().position(|m| m.start >= after);
        if let Some(next) = next.or((!self.search.matches.is_empty()).then_some(0)) {
            self.search.current = next;
            self.select_current_match(&document.text);
        }
    }

    fn replace_all(&mut self, document: &mut Document) {
        self.update_matches(&document.text);
        if self.search.matches.is_empty() {
            return;
        }
        let selection = self.clamped_selection(&document.text);
        let edited = text_ops::replace_ranges(
            &document.text,
            &selection,
            &self.search.matches,
            &self.search.replacement,
        );
        self.apply(document, edited);
        self.pending_focus = false;
    }

    fn go_to_bar(&mut self, ui: &mut egui::Ui, document: &Document) {
        use egui_phosphor::regular as icon;

        let Some(go_to) = &mut self.go_to else {
            return;
        };
        let lines = document.text.matches('\n').count() + 1;
        let target = text_ops::go_to_target(&document.text, &go_to.input);
        let mut jump = false;
        let mut close = false;
        let field_width = bar_field_width(ui);
        ui.horizontal(|ui| {
            ui.label(icon::LIST_NUMBERS);
            let response = ui.add(
                TextEdit::singleline(&mut go_to.input)
                    .hint_text(format!("Line (1–{lines}), or line:column"))
                    .desired_width(field_width),
            );
            if std::mem::take(&mut go_to.focus) {
                response.request_focus();
            }
            if response.has_focus() || response.lost_focus() {
                let (enter, _, escape) = take_bar_keys(ui);
                jump |= enter;
                close |= escape;
            }
            if ui
                .add_enabled(target.is_some(), egui::Button::new("Go"))
                .clicked()
            {
                jump = true;
            }
            if !go_to.input.trim().is_empty() && target.is_none() {
                ui.weak("Not a line number");
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                close |= ui.button(icon::X).on_hover_text("Close (Esc)").clicked();
            });
        });
        ui.separator();

        if jump && let Some((line, offset)) = target {
            self.pending_selection = Some(Selection::caret(offset));
            self.pending_scroll_line = Some(line + 1);
            close = true;
        }
        if close {
            self.go_to = None;
            self.pending_focus = true;
        }
    }

    fn update_matches(&mut self, text: &str) {
        let mut hasher = DefaultHasher::new();
        let search = &self.search;
        (text, &search.query, search.match_case, search.whole_word).hash(&mut hasher);
        let key = hasher.finish() | 1;
        if self.search.computed_for == key {
            return;
        }
        self.search.computed_for = key;
        self.search.matches = find_all(
            text,
            &self.search.query,
            self.search.match_case,
            self.search.whole_word,
        );
        if self.search.current >= self.search.matches.len() {
            self.search.current = 0;
        }
    }

    fn select_current_match(&mut self, text: &str) {
        if let Some(range) = self.search.matches.get(self.search.current).cloned() {
            self.search.visited = Some((range.clone(), self.search.computed_for));
            self.pending_selection = Some(Selection {
                anchor: range.start,
                cursor: range.end,
            });
            self.pending_scroll_line = Some(text_ops::line_of(text, range.start) + 1);
        }
    }

    fn backgrounds(
        &self,
        geometry: &Geometry,
        text: &str,
        options: &EditorOptions<'_>,
        text_left: f32,
        clip: Rect,
        char_width: f32,
    ) -> Vec<Shape> {
        let colors = options.colors;
        let mut shapes = Vec::new();
        let full_width = |rect: Rect| {
            Rect::from_x_y_ranges(text_left..=clip.right().max(rect.right()), rect.y_range())
        };

        let mut line_background = |line: Option<usize>, color: Color32| {
            if let Some(rect) = line.and_then(|l| geometry.line_rect(l.saturating_sub(1)))
                && rect.intersects(clip)
            {
                shapes.push(Shape::rect_filled(full_width(rect), 0.0, color));
            }
        };
        if options.highlight_current_line && self.selection.range().is_empty() {
            line_background(
                Some(text_ops::line_of(text, self.selection.cursor) + 1),
                colors.current_line,
            );
        }
        line_background(self.error_line, colors.error_line);
        line_background(self.debug_line, colors.debug_line);

        if self.search.open {
            for (i, range) in self
                .search
                .matches
                .iter()
                .enumerate()
                .take(MAX_HIGHLIGHTED_MATCHES)
            {
                let start = geometry.caret_rect(text, range.start);
                let end = geometry.caret_rect(text, range.end);
                if !start.intersects(clip) {
                    continue;
                }
                let rect = if (start.top() - end.top()).abs() < 1.0 {
                    Rect::from_min_max(start.left_top(), Pos2::new(end.left(), start.bottom()))
                } else {
                    Rect::from_min_max(
                        start.left_top(),
                        Pos2::new(start.left() + char_width, start.bottom()),
                    )
                };
                let color = if i == self.search.current {
                    colors.search_current
                } else {
                    colors.search_match
                };
                shapes.push(Shape::rect_filled(
                    rect.expand2(Vec2::new(1.0, 0.0)),
                    2.0,
                    color,
                ));
            }
        }

        if options.ruler > 0 {
            let x = geometry.origin.x + options.ruler as f32 * char_width;
            if (clip.left()..=clip.right()).contains(&x) {
                shapes.push(Shape::line_segment(
                    [Pos2::new(x, clip.top()), Pos2::new(x, clip.bottom())],
                    Stroke::new(1.0, colors.ruler),
                ));
            }
        }
        shapes
    }

    fn paint_gutter(
        &self,
        ui: &egui::Ui,
        geometry: &Geometry,
        gutter: Rect,
        colors: &EditorColors,
        number_width: f32,
    ) {
        let painter = ui.painter_at(gutter);
        let clip = gutter;
        let font = FontId::monospace(geometry.row_height.max(8.0) * 0.8);
        let cursor_line = geometry.line_of_byte(self.selection.cursor);
        let slowest = self
            .line_profile
            .values()
            .map(|timing| timing.seconds)
            .fold(0.0, f64::max);
        let time_right = gutter.right() - 10.0 - number_width - 8.0;
        for (line, rect) in geometry.lines() {
            if rect.bottom() < clip.top() || rect.top() > clip.bottom() {
                continue;
            }
            let number = line + 1;
            let y_center = rect.top() + geometry.row_height / 2.0;
            let color = if self.error_line == Some(number) {
                colors.error_text
            } else if Some(line) == cursor_line {
                colors.gutter_text_current
            } else {
                colors.gutter_text
            };
            painter.text(
                Pos2::new(gutter.right() - 10.0, rect.top()),
                Align2::RIGHT_TOP,
                number.to_string(),
                font.clone(),
                color,
            );
            if let Some(timing) = self.line_profile.get(&number)
                && timing.seconds >= PROFILE_FLOOR_SECONDS
            {
                // Hotter for a larger share of the slowest line's time.
                let share = (timing.seconds / slowest.max(f64::EPSILON)).sqrt() as f32;
                let color = lerp_color(colors.gutter_text, PROFILE_HOT, share);
                let galley = painter.layout_no_wrap(
                    format_seconds(timing.seconds),
                    FontId::monospace(font.size * 0.9),
                    color,
                );
                let at = Pos2::new(
                    time_right - galley.size().x,
                    y_center - galley.size().y / 2.0,
                );
                let area = Rect::from_min_size(at, galley.size());
                painter.galley(at, galley, color);
                ui.interact(area, self.text_id.with(("profile", number)), Sense::hover())
                    .on_hover_text(profile_tooltip(number, timing));
            }
            let marker = Pos2::new(gutter.left() + 10.0, y_center);
            if self.breakpoints.contains(&number) {
                painter.circle(
                    marker,
                    4.5,
                    colors.breakpoint,
                    Stroke::new(1.0, colors.breakpoint.gamma_multiply(0.6)),
                );
            }
            if self.debug_line == Some(number) {
                let arrow = vec![
                    marker + Vec2::new(-4.0, -5.0),
                    marker + Vec2::new(5.0, 0.0),
                    marker + Vec2::new(-4.0, 5.0),
                ];
                painter.add(Shape::convex_polygon(
                    arrow,
                    colors.debug_marker,
                    Stroke::NONE,
                ));
            }
        }
    }

    fn popups(&mut self, ui: &egui::Ui, document: &mut Document, caret: Rect, row_height: f32) {
        let anchor = caret.left_bottom() + Vec2::new(0.0, 2.0);
        if self.completion.open {
            let visible = self.visible_completions(&document.text);
            if visible.is_empty() {
                self.completion.open = false;
            } else {
                self.completion.selected = self.completion.selected.min(visible.len() - 1);
                let mut clicked = None;
                let scroll = std::mem::take(&mut self.completion.scroll_to_selected);
                egui::Area::new(self.text_id.with("completion"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(anchor)
                    .show(ui.ctx(), |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.set_min_width(220.0);
                            egui::ScrollArea::vertical()
                                .max_height(row_height * COMPLETION_ROWS as f32 * 1.6)
                                .show(ui, |ui| {
                                    for (position, &index) in visible.iter().enumerate() {
                                        let item = &self.completion.items[index];
                                        let selected = position == self.completion.selected;
                                        let label =
                                            format!("{}  {}", kind_icon(&item.kind), item.name);
                                        let response = ui.selectable_label(selected, label);
                                        if selected && scroll {
                                            response.scroll_to_me(None);
                                        }
                                        if response.clicked() {
                                            clicked = Some(index);
                                        }
                                    }
                                });
                        });
                    });
                if let Some(index) = clicked {
                    self.accept_completion(document, index);
                }
            }
        }
        if let Some(calltip) = &self.calltip {
            egui::Area::new(self.text_id.with("calltip"))
                .order(egui::Order::Foreground)
                .fixed_pos(anchor)
                .interactable(false)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.monospace(calltip);
                    });
                });
        }
    }
}

/// Width of the text field of the search, replace and *Go to line* bars:
/// as wide as the buttons next to it leave room for in a narrow editor.
fn bar_field_width(ui: &egui::Ui) -> f32 {
    (ui.available_width() - 320.0).clamp(100.0, 220.0)
}

/// Takes Enter and Escape from this frame's input: a bar's text field acts
/// on them, and the editor, which may get the keyboard back in this frame,
/// must not see them too (Enter would replace its selection with a new
/// line). Returns whether Enter was pressed, with Shift held, and Escape.
fn take_bar_keys(ui: &egui::Ui) -> (bool, bool, bool) {
    ui.input_mut(|i| {
        let shift = i.modifiers.shift;
        let enter = i.consume_key(Modifiers::NONE, Key::Enter);
        let escape = i.consume_key(Modifiers::NONE, Key::Escape);
        (enter, shift, escape)
    })
}

/// Whether typing has just reached `min_chars` characters of a name at
/// `cursor`, the moment to offer completions. Names never start with a
/// digit, and words in comments and strings are not names; 0 turns this
/// off.
fn completes_name(text: &str, cursor: usize, min_chars: usize) -> bool {
    let name = text_ops::name_before(text, cursor);
    min_chars > 0
        && name.chars().count() == min_chars
        && !name.starts_with(|c: char| c.is_ascii_digit())
        && highlight::in_code(text, cursor)
}

/// The name whose characters are under the pointer at `pos`.
fn name_at(galley: &Galley, origin: Pos2, text: &str, pos: Pos2) -> Option<Range<usize>> {
    let caret = galley.cursor_from_pos(pos - origin);
    let byte = text_ops::char_to_byte(text, caret.index.0);
    // The caret is between two characters: the pointer is on the one after
    // it when it is to the right of the caret.
    let caret_x = galley.pos_from_cursor(caret).left() + origin.x;
    let on = if pos.x >= caret_x {
        byte
    } else {
        text[..byte].char_indices().next_back()?.0
    };
    let name = text_ops::word_at(text, on)?;
    let rect = |byte: usize| {
        galley
            .pos_from_cursor(CCursor::new(text_ops::byte_to_char(text, byte)))
            .translate(origin.to_vec2())
    };
    let (start, end) = (rect(name.start), rect(name.end));
    let inside = (start.top() - end.top()).abs() < 1.0
        && (start.left()..=end.left()).contains(&pos.x)
        && start.y_range().contains(pos.y);
    inside.then_some(name)
}

/// Text typed between `anchor` and `cursor` (empty if the cursor moved back).
fn typed_since(text: &str, anchor: usize, cursor: usize) -> &str {
    typed_text(text, anchor, cursor).unwrap_or_default()
}

/// Text between `anchor` and `cursor`, or `None` when the cursor moved back
/// or an edit elsewhere left the anchor outside the text or inside a
/// character, where slicing would panic.
fn typed_text(text: &str, anchor: usize, cursor: usize) -> Option<&str> {
    (anchor <= cursor && text.is_char_boundary(anchor) && text.is_char_boundary(cursor))
        .then(|| &text[anchor..cursor])
}

fn kind_icon(kind: &str) -> &'static str {
    use egui_phosphor::regular as icon;
    match kind {
        "function" => icon::FUNCTION,
        "class" => icon::CUBE,
        "module" => icon::PACKAGE,
        "keyword" => icon::TEXT_AA,
        "param" => icon::AT,
        "property" | "statement" | "instance" => icon::DOT_OUTLINE,
        _ => icon::DOT,
    }
}

fn layout_key(text: &str, wrap_width: f32, font_size: f32, dark: bool) -> u64 {
    let mut hasher = DefaultHasher::new();
    (text, wrap_width.to_bits(), font_size.to_bits(), dark).hash(&mut hasher);
    hasher.finish()
}

fn floor_char_boundary(text: &str, mut index: usize) -> usize {
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

/// The match to go to from `selection`: the first one after it going
/// forward, the last one before it going back, wrapping around.
fn next_match(matches: &[Range<usize>], selection: &Range<usize>, forward: bool) -> Option<usize> {
    if matches.is_empty() {
        return None;
    }
    Some(if forward {
        matches
            .iter()
            .position(|m| m.start >= selection.end && m != selection)
            .unwrap_or(0)
    } else {
        matches
            .iter()
            .rposition(|m| m.end <= selection.start && m != selection)
            .unwrap_or(matches.len() - 1)
    })
}

/// Whether `range` is a whole word: no name character touches either end.
fn is_whole_word(text: &str, range: &Range<usize>) -> bool {
    let before = text[..range.start].chars().next_back();
    let after = text[range.end..].chars().next();
    !before.is_some_and(text_ops::is_word_char) && !after.is_some_and(text_ops::is_word_char)
}

/// Byte ranges of every occurrence of `query`, optionally only where it is
/// a whole word.
fn find_all(text: &str, query: &str, match_case: bool, whole_word: bool) -> Vec<Range<usize>> {
    let mut matches = find_occurrences(text, query, match_case);
    if whole_word {
        matches.retain(|range| is_whole_word(text, range));
    }
    matches
}

fn find_occurrences(text: &str, query: &str, match_case: bool) -> Vec<Range<usize>> {
    if query.is_empty() {
        return Vec::new();
    }
    if match_case {
        return text
            .match_indices(query)
            .map(|(i, m)| i..i + m.len())
            .collect();
    }
    // Lowercasing can change byte lengths for some scripts, so compare
    // character by character.
    let needle: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    let haystack: Vec<(usize, char)> = text
        .char_indices()
        .flat_map(|(i, c)| c.to_lowercase().map(move |l| (i, l)))
        .collect();
    let mut matches = Vec::new();
    let mut i = 0;
    while i + needle.len() <= haystack.len() {
        if haystack[i..i + needle.len()]
            .iter()
            .map(|(_, c)| *c)
            .eq(needle.iter().copied())
        {
            let start = haystack[i].0;
            let last = haystack[i + needle.len() - 1].0;
            let end = last + text[last..].chars().next().map_or(0, char::len_utf8);
            matches.push(start..end);
            i += needle.len();
        } else {
            i += 1;
        }
    }
    matches
}

/// Where lines and characters ended up on screen.
struct Geometry {
    galley: Arc<Galley>,
    origin: Pos2,
    row_height: f32,
    /// First row index of each logical line.
    line_rows: Vec<usize>,
}

impl Geometry {
    fn new(galley: &Arc<Galley>, origin: Pos2) -> Self {
        let mut line_rows = vec![0];
        for (index, row) in galley.rows.iter().enumerate() {
            if row.ends_with_newline {
                line_rows.push(index + 1);
            }
        }
        let row_height = galley.rows.first().map_or(14.0, |row| row.rect().height());
        Self {
            galley: galley.clone(),
            origin,
            row_height,
            line_rows,
        }
    }

    /// Screen rectangle of a (0-based) logical line, across its wrapped rows.
    fn line_rect(&self, line: usize) -> Option<Rect> {
        let first = *self.line_rows.get(line)?;
        let last = self
            .line_rows
            .get(line + 1)
            .map_or(self.galley.rows.len(), |&next| next)
            .max(first + 1);
        let top = self
            .galley
            .rows
            .get(first)
            .map_or_else(|| self.galley.rect.bottom(), |row| row.rect().top());
        let bottom = self
            .galley
            .rows
            .get(last - 1)
            .map_or(top + self.row_height, |row| row.rect().bottom());
        Some(Rect::from_min_max(
            Pos2::new(self.origin.x, self.origin.y + top),
            Pos2::new(
                self.origin.x + self.galley.rect.width(),
                self.origin.y + bottom,
            ),
        ))
    }

    fn lines(&self) -> impl Iterator<Item = (usize, Rect)> + '_ {
        (0..self.line_rows.len()).filter_map(|line| self.line_rect(line).map(|rect| (line, rect)))
    }

    fn line_at(&self, y: f32) -> Option<usize> {
        self.lines()
            .find(|(_, rect)| rect.y_range().contains(y))
            .map(|(line, _)| line)
    }

    fn line_of_byte(&self, byte: usize) -> Option<usize> {
        let text = self.galley.text();
        Some(text_ops::line_of(text, byte.min(text.len())))
    }

    /// Screen rectangle of the caret at `byte`.
    fn caret_rect(&self, text: &str, byte: usize) -> Rect {
        let index = text_ops::byte_to_char(text, byte.min(text.len()));
        self.galley
            .pos_from_cursor(CCursor::new(index))
            .translate(self.origin.to_vec2())
    }
}

#[cfg(test)]
#[path = "../../tests/rust/editor/test_editor.rs"]
mod tests;

/// The gutter's color for the slowest lines.
const PROFILE_HOT: Color32 = Color32::from_rgb(235, 110, 45);

fn lerp_color(from: Color32, to: Color32, amount: f32) -> Color32 {
    let mix = |a: u8, b: u8| {
        (f32::from(a) + (f32::from(b) - f32::from(a)) * amount.clamp(0.0, 1.0)).round() as u8
    };
    Color32::from_rgb(
        mix(from.r(), to.r()),
        mix(from.g(), to.g()),
        mix(from.b(), to.b()),
    )
}

fn profile_tooltip(line: usize, timing: &LineTiming) -> String {
    let times = if timing.hits == 1 {
        String::new()
    } else {
        format!(" over {} runs of the line", timing.hits)
    };
    let mut text = format!("Line {line}: {}{times}", format_seconds(timing.seconds));
    for (name, seconds, calls) in &timing.operations {
        let calls = if *calls == 1 {
            String::new()
        } else {
            format!(" ({calls} calls)")
        };
        text += &format!("\n{name}: {}{calls}", format_seconds(*seconds));
    }
    text
}
