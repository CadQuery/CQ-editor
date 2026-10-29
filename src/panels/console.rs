//! Interactive Python console running in the worker.
//!
//! Its namespace has the script API (`show_object` adds to the view) and the
//! variables of the last run. Enter runs the input, Shift+Enter adds a line,
//! Up/Down browse the history. Results with an image (a matplotlib figure,
//! anything with `_repr_png_`) show it, decoded before it gets here.

use egui::{Color32, Key, Modifiers, RichText};
use egui_phosphor::regular as icon;

use crate::app::AppState;
use crate::commands::Action;
use crate::worker::protocol::{DisplayImage, DisplayOutput};

const MAX_ENTRIES: usize = 2000;
/// Images kept; older ones are replaced by a note, to bound memory.
const MAX_IMAGES: usize = 50;
/// Decoded pixels retained on the CPU. Textures use additional GPU memory.
const MAX_IMAGE_MEMORY: usize = 128 * 1024 * 1024;
/// Images are drawn at most this tall, in points.
const MAX_IMAGE_HEIGHT: f32 = 360.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Input,
    Output,
    Error,
    Info,
}

pub struct Entry {
    pub kind: EntryKind,
    pub text: String,
    /// A rich result's picture, shown above its text.
    pub image: Option<ConsoleImage>,
}

/// An image in the console; its texture is made when it is first drawn.
pub struct ConsoleImage {
    pixels: DisplayImage,
    texture: Option<egui::TextureHandle>,
}

#[derive(Default)]
pub struct ConsoleState {
    pub entries: Vec<Entry>,
    pub input: String,
    /// The last statement is incomplete (a block is open).
    pub more: bool,
    pub busy: bool,
    history: Vec<String>,
    history_index: Option<usize>,
    /// In-progress input saved before browsing or searching history.
    draft_input: Option<String>,
    /// Whether the interactive history search bar (`Ctrl+R`) is open.
    pub search_open: bool,
    pub search_query: String,
    pub search_match_index: usize,
    focus_search: bool,
    focus_input: bool,
}

impl ConsoleState {
    #[cfg(test)]
    pub fn history(&self) -> &[String] {
        &self.history
    }

    /// Indices into `self.history` (most recent first) matching `query`
    /// case-insensitively.
    pub fn history_matches(&self, query: &str) -> Vec<usize> {
        let needle = query.trim().to_lowercase();
        (0..self.history.len())
            .rev()
            .filter(|&i| needle.is_empty() || self.history[i].to_lowercase().contains(&needle))
            .collect()
    }

    /// Opens the reverse history search bar (`Ctrl+R`), saving the current
    /// input so `Esc` can restore it.
    pub fn open_history_search(&mut self) {
        if !self.search_open {
            self.draft_input = Some(self.input.clone());
        }
        self.search_open = true;
        self.focus_search = true;
        self.search_match_index = 0;
        if !self.search_query.trim().is_empty() {
            self.apply_search_match();
        }
    }

    /// Updates the history search query and loads the newest matching command.
    pub fn set_history_search_query(&mut self, query: &str) {
        if !self.search_open {
            self.draft_input = Some(self.input.clone());
            self.search_open = true;
        }
        self.search_query = query.to_owned();
        self.search_match_index = 0;
        if self.search_query.trim().is_empty() {
            if let Some(draft) = &self.draft_input {
                self.input = draft.clone();
            }
            self.history_index = None;
        } else {
            self.apply_search_match();
        }
    }

    /// Cycles to the next older (`true`) or newer (`false`) matching command.
    pub fn step_history_search(&mut self, next_older: bool) {
        let matches = self.history_matches(&self.search_query);
        if matches.is_empty() {
            return;
        }
        if next_older {
            self.search_match_index = (self.search_match_index + 1) % matches.len();
        } else {
            self.search_match_index = (self.search_match_index + matches.len() - 1) % matches.len();
        }
        let hist_idx = matches[self.search_match_index];
        self.history_index = Some(hist_idx);
        self.input = self.history[hist_idx].clone();
    }

    /// Accepts the currently matched history command into the input field
    /// without executing it.
    pub fn accept_history_search(&mut self) {
        self.search_open = false;
        self.focus_search = false;
        self.draft_input = None;
        self.focus_input = true;
    }

    /// Cancels history search and restores the input that was present before
    /// the search started.
    pub fn cancel_history_search(&mut self) {
        self.search_open = false;
        self.focus_search = false;
        if let Some(draft) = self.draft_input.take() {
            self.input = draft;
        }
        self.history_index = None;
        self.focus_input = true;
    }

    fn apply_search_match(&mut self) {
        let matches = self.history_matches(&self.search_query);
        if let Some(&hist_idx) = matches.get(self.search_match_index) {
            self.history_index = Some(hist_idx);
            self.input = self.history[hist_idx].clone();
        }
    }
    /// Applies completion results for the input: a unique match or the
    /// common prefix is inserted, otherwise the candidates are listed.
    pub fn complete(&mut self, inserts: &[(String, String)], signatures: &[String]) {
        if inserts.is_empty() {
            if !signatures.is_empty() {
                self.push(EntryKind::Info, &(signatures.join("\n") + "\n"));
            }
            return;
        }
        let prefix = common_prefix(inserts.iter().map(|(_, insert)| insert.as_str()));
        if !prefix.is_empty() {
            self.input.push_str(prefix);
        } else if inserts.len() > 1 {
            let names: Vec<&str> = inserts.iter().map(|(name, _)| name.as_str()).collect();
            self.push(EntryKind::Info, &(names.join("  ") + "\n"));
        }
        self.focus_input = true;
    }

    pub fn push(&mut self, kind: EntryKind, text: &str) {
        // Consecutive output chunks are merged into one entry.
        if let Some(last) = self.entries.last_mut()
            && last.kind == kind
            && kind != EntryKind::Input
            && last.image.is_none()
            && !last.text.ends_with('\n')
        {
            last.text.push_str(text);
            return;
        }
        self.add(Entry {
            kind,
            text: text.to_owned(),
            image: None,
        });
    }

    /// A rich result of a command: its image, if any, then its text.
    pub fn push_display(&mut self, output: DisplayOutput) {
        let text = if output.text.is_empty() {
            String::new()
        } else {
            output.text + "\n"
        };
        match output.image {
            Some(pixels) => {
                self.add(Entry {
                    kind: EntryKind::Output,
                    text,
                    image: Some(ConsoleImage {
                        pixels,
                        texture: None,
                    }),
                });
                self.trim_images(MAX_IMAGES, MAX_IMAGE_MEMORY);
            }
            None if !text.is_empty() => self.push(EntryKind::Output, &text),
            None => {}
        }
    }

    fn add(&mut self, entry: Entry) {
        self.entries.push(entry);
        if self.entries.len() > MAX_ENTRIES {
            self.entries.drain(..self.entries.len() - MAX_ENTRIES);
        }
    }

    fn trim_images(&mut self, max_images: usize, max_bytes: usize) {
        let mut images = self
            .entries
            .iter()
            .filter(|entry| entry.image.is_some())
            .count();
        let mut bytes: usize = self
            .entries
            .iter()
            .filter_map(|entry| entry.image.as_ref())
            .map(|image| image.pixels.rgba.len())
            .sum();
        for entry in &mut self.entries {
            if images <= max_images && bytes <= max_bytes {
                break;
            }
            if let Some(image) = entry.image.take() {
                entry.text.insert_str(0, "[image]\n");
                images -= 1;
                bytes = bytes.saturating_sub(image.pixels.rgba.len());
            }
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn record_input(&mut self, source: &str) {
        let prompt = if self.more { "... " } else { ">>> " };
        let shown: Vec<String> = source
            .lines()
            .enumerate()
            .map(|(i, line)| format!("{}{line}", if i == 0 { prompt } else { "... " }))
            .collect();
        self.push(EntryKind::Input, &(shown.join("\n") + "\n"));
        if !source.trim().is_empty() && self.history.last().map(String::as_str) != Some(source) {
            self.history.push(source.to_owned());
        }
        self.history_index = None;
        self.draft_input = None;
        self.search_open = false;
        self.busy = true;
    }

    /// Moves one step through the command history (`older = true` for `Up`,
    /// `false` for `Down`), preserving any in-progress draft input.
    pub fn step_history(&mut self, older: bool) -> bool {
        if self.history.is_empty() || (self.input.contains('\n') && self.history_index.is_none()) {
            return false;
        }
        if older {
            if self.history_index.is_none() {
                self.draft_input = Some(self.input.clone());
            }
            let index = self
                .history_index
                .map_or(self.history.len() - 1, |i| i.saturating_sub(1));
            self.history_index = Some(index);
            self.input = self.history[index].clone();
            true
        } else {
            match self.history_index {
                Some(i) if i + 1 < self.history.len() => {
                    self.history_index = Some(i + 1);
                    self.input = self.history[i + 1].clone();
                    true
                }
                Some(_) => {
                    self.history_index = None;
                    self.input = self.draft_input.take().unwrap_or_default();
                    true
                }
                None => false,
            }
        }
    }
}

fn common_prefix<'a>(mut words: impl Iterator<Item = &'a str>) -> &'a str {
    let Some(first) = words.next() else { return "" };
    let mut length = first.len();
    for word in words {
        length = first
            .char_indices()
            .zip(word.chars())
            .take_while(|((_, a), b)| a == b)
            .last()
            .map_or(0, |((i, c), _)| i + c.len_utf8())
            .min(length);
    }
    &first[..length]
}

pub fn ui(ui: &mut egui::Ui, state: &mut AppState) {
    let mut actions = Vec::new();
    let colors = Colors::new(ui.visuals());
    let worker_ready = state.worker.is_ready();
    let console = &mut state.console;

    egui::Panel::bottom("console_input")
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(6, 4)))
        .show(ui, |ui| {
            if console.search_open {
                history_search_bar(ui, console);
                ui.add_space(4.0);
            }
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(if console.more { "..." } else { ">>>" })
                        .monospace()
                        .color(colors.prompt),
                );
                let input_id = ui.id().with("console_input_text");
                let focused = ui.memory(|m| m.has_focus(input_id));
                if focused {
                    handle_keys(ui, console, &mut actions);
                }
                let response = ui.add_enabled(
                    worker_ready,
                    egui::TextEdit::multiline(&mut console.input)
                        .id(input_id)
                        .font(egui::TextStyle::Monospace)
                        .desired_rows(1)
                        // Room for the buttons, Stop included while busy.
                        .desired_width(
                            ui.available_width() - if console.busy { 120.0 } else { 90.0 },
                        )
                        .hint_text(if worker_ready {
                            "Python — Enter to run · Ctrl+R to search history"
                        } else {
                            "Waiting for Python…"
                        }),
                );
                if std::mem::take(&mut console.focus_input) {
                    response.request_focus();
                }
                if console.busy
                    && ui
                        .button(icon::STOP)
                        .on_hover_text(
                            "Stop the running command (Ctrl+Shift+F5); its variables stay",
                        )
                        .clicked()
                {
                    actions.push(Action::Stop);
                }
                if ui
                    .add_enabled(
                        !console.history.is_empty(),
                        egui::Button::new(icon::MAGNIFYING_GLASS).selected(console.search_open),
                    )
                    .on_hover_text("Search command history (Ctrl+R)")
                    .clicked()
                {
                    if console.search_open {
                        console.cancel_history_search();
                    } else {
                        console.open_history_search();
                    }
                }
                if ui
                    .button(icon::BROOM)
                    .on_hover_text("Clear the console")
                    .clicked()
                {
                    actions.push(Action::ClearConsole);
                }
                if ui
                    .button(icon::ARROW_COUNTER_CLOCKWISE)
                    .on_hover_text("Restart the console (forgets its variables)")
                    .clicked()
                {
                    actions.push(Action::ResetConsole);
                }
            });
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            output(ui, console, &colors);
        });

    state.queue(actions);
}

fn output(ui: &mut egui::Ui, console: &mut ConsoleState, colors: &Colors) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(8, 4))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    if console.entries.is_empty() {
                        ui.weak(
                            "Python console. Try: show_object(cq.Workplane().sphere(5)) · \
                             selected() returns what is picked in the 3D view",
                        );
                    }
                    for entry in &mut console.entries {
                        if let Some(image) = &mut entry.image {
                            let texture = image.texture.get_or_insert_with(|| {
                                ui.ctx().load_texture(
                                    "console image",
                                    egui::ColorImage::from_rgba_unmultiplied(
                                        image.pixels.size,
                                        &image.pixels.rgba,
                                    ),
                                    egui::TextureOptions::LINEAR,
                                )
                            });
                            let size = texture.size_vec2();
                            let scale = (ui.available_width() / size.x)
                                .min(MAX_IMAGE_HEIGHT / size.y)
                                .min(1.0);
                            ui.add_space(4.0);
                            ui.image((texture.id(), size * scale));
                            ui.add_space(4.0);
                        }
                        if entry.text.is_empty() {
                            continue;
                        }
                        let color = match entry.kind {
                            EntryKind::Input => colors.input,
                            EntryKind::Output => colors.output,
                            EntryKind::Error => colors.error,
                            EntryKind::Info => colors.info,
                        };
                        let text = entry.text.strip_suffix('\n').unwrap_or(&entry.text);
                        ui.add(
                            egui::Label::new(RichText::new(text).monospace().color(color))
                                .selectable(true),
                        );
                    }
                    if console.busy {
                        ui.spinner();
                    }
                });
        });
}

fn history_search_bar(ui: &mut egui::Ui, console: &mut ConsoleState) {
    ui.horizontal(|ui| {
        ui.label(icon::MAGNIFYING_GLASS);
        let search_id = ui.id().with("console_history_search");
        let focused = ui.memory(|m| m.has_focus(search_id));
        if focused {
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                console.cancel_history_search();
                return;
            }
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
                console.accept_history_search();
                return;
            }
            if ui.input_mut(|i| {
                i.consume_key(Modifiers::COMMAND, Key::R)
                    || i.consume_key(Modifiers::NONE, Key::ArrowUp)
            }) {
                console.step_history_search(true);
            } else if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown)) {
                console.step_history_search(false);
            }
        }
        let mut query = console.search_query.clone();
        let response = ui.add(
            egui::TextEdit::singleline(&mut query)
                .id(search_id)
                .desired_width((ui.available_width() - 150.0).max(80.0))
                .hint_text("Search history… (Enter to use, Esc to cancel)"),
        );
        if std::mem::take(&mut console.focus_search) {
            response.request_focus();
        }
        if response.changed() {
            console.set_history_search_query(&query);
        }
        let matches = console.history_matches(&console.search_query);
        if matches.is_empty() {
            ui.weak("No match");
        } else {
            let current = console.search_match_index.min(matches.len() - 1) + 1;
            ui.weak(format!("{current}/{}", matches.len()));
        }
        if ui
            .add_enabled(!matches.is_empty(), egui::Button::new(icon::CARET_UP))
            .on_hover_text("Older match (Ctrl+R / Up)")
            .clicked()
        {
            console.step_history_search(true);
        }
        if ui
            .add_enabled(!matches.is_empty(), egui::Button::new(icon::CARET_DOWN))
            .on_hover_text("Newer match (Down)")
            .clicked()
        {
            console.step_history_search(false);
        }
        if ui
            .button(icon::CHECK)
            .on_hover_text("Use command in input (Enter)")
            .clicked()
        {
            console.accept_history_search();
        }
        if ui
            .button(icon::X)
            .on_hover_text("Cancel search and restore previous input (Esc)")
            .clicked()
        {
            console.cancel_history_search();
        }
    });
}

fn handle_keys(ui: &egui::Ui, console: &mut ConsoleState, actions: &mut Vec<Action>) {
    if ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::R)) {
        if console.search_open {
            console.step_history_search(true);
        } else {
            console.open_history_search();
        }
        return;
    }
    let submit = ui.input_mut(|i| {
        // Shift+Enter falls through to the text edit and inserts a newline.
        !i.modifiers.shift && i.consume_key(Modifiers::NONE, Key::Enter)
    });
    if submit {
        let source = std::mem::take(&mut console.input);
        actions.push(Action::ConsoleSubmit(source));
        console.focus_input = true;
        return;
    }
    if !console.input.is_empty() && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab)) {
        actions.push(Action::ConsoleComplete);
        return;
    }
    if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp)) {
        console.step_history(true);
    } else if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown)) {
        console.step_history(false);
    }
}

struct Colors {
    prompt: Color32,
    input: Color32,
    output: Color32,
    error: Color32,
    info: Color32,
}

impl Colors {
    fn new(visuals: &egui::Visuals) -> Self {
        let text = visuals.text_color();
        Self {
            prompt: visuals.hyperlink_color,
            input: visuals.hyperlink_color.lerp_to_gamma(text, 0.35),
            output: text,
            error: visuals.error_fg_color,
            info: visuals.weak_text_color(),
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rust/panels/test_console.rs"]
mod tests;
