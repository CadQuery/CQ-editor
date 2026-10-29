//! Everything the user can ask for, from menus, the toolbar, panels and
//! keyboard shortcuts. Widgets only queue actions; the `app` module
//! executes them after the frame's UI has been built.

use std::path::PathBuf;

use egui::{Key, KeyboardShortcut, Modifiers};

use crate::panels::Tab;
use crate::scene::{NodeKey, TopologySelection};
use crate::settings::SelectionShortcuts;
use crate::viewer::camera::StandardView;
use crate::viewer::{DisplayMode, SelectionMode};
use crate::worker::protocol::{DebugAction, ExportFormat};

/// Structured formats for copying exact measurements to the clipboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeasurementFormat {
    Text,
    Json,
    Csv,
}

impl MeasurementFormat {
    pub const ALL: [Self; 3] = [Self::Text, Self::Json, Self::Csv];

    pub fn label(self) -> &'static str {
        match self {
            Self::Text => "Text",
            Self::Json => "JSON",
            Self::Csv => "CSV",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    // File
    NewFile,
    OpenFile,
    OpenPath(PathBuf),
    OpenExample(usize),
    Save,
    SaveAs,
    /// Saves every tab with unsaved changes.
    SaveAll,
    LocalHistory,
    ToggleAutoreload,
    Quit,
    // Tabs
    /// Closes the shown tab.
    CloseTab,
    /// Closes the tab of this document (by generation).
    CloseDocument(u64),
    CloseOtherTabs(u64),
    CloseAllTabs,
    ActivateTab(u64),
    NextTab,
    PreviousTab,
    // Project
    /// Asks for a folder and shows it in the Project panel.
    OpenFolder,
    OpenFolderPath(PathBuf),
    CloseFolder,
    /// Reads the project's folders again.
    RefreshProject,
    /// Asks for the name of a new file in this folder.
    NewFileIn(PathBuf),
    NewFolderIn(PathBuf),
    /// Asks for a new name of this file or folder.
    RenamePath(PathBuf),
    /// Asks to confirm deleting this file or folder.
    DeletePath(PathBuf),
    /// The script F5 runs while a file of the project is shown; `None`
    /// runs the shown file.
    SetMainScript(Option<PathBuf>),
    /// Opens the system's file manager on this file's folder.
    RevealPath(PathBuf),
    // Edit
    Undo,
    Redo,
    ToggleComment,
    DuplicateLine,
    MoveLineUp,
    MoveLineDown,
    InsertSnippet(usize),
    Autocomplete,
    Find,
    FindNext,
    FindPrevious,
    Replace,
    /// Opens the *Go to line* bar.
    GoToLinePrompt,
    ShowOutline,
    GoToDefinition,
    FindReferences,
    /// The CadQuery selector of the picked entities, to the clipboard.
    CopySelector,
    /// The CadQuery selector of the picked entities, at the editor's cursor.
    InsertSelector,
    /// Python repeating the current measurement, to the clipboard.
    CopyMeasurementCode,
    /// Copies the current measurement as structured data (Text, JSON, CSV).
    CopyMeasurement(MeasurementFormat),
    Preferences,
    // Run
    Render,
    /// Renders with the time of each line measured, shown in the gutter.
    Profile,
    /// Removes the profile's times from the gutter.
    ClearProfile,
    /// Starts a debugging session, or ends the running one.
    ToggleDebug,
    Debug(DebugAction),
    /// Aborts whatever the worker is doing (by restarting it).
    Stop,
    /// Stops meshing the running render's objects; the model shown stays.
    CancelMeshing,
    ClearConsole,
    ClearLog,
    RestartPython,
    /// Restarts Python with this interpreter and keeps it in the
    /// preferences.
    UsePython(PathBuf),
    /// Opens the dialog creating a Python environment with CadQuery.
    CreateEnvironment,
    /// Opens the dialog installing packages into the Python in use.
    InstallPackages,
    // View
    Fit,
    FitSelection,
    View(StandardView),
    Display(DisplayMode),
    SelectionMode(SelectionMode),
    ToggleProjection,
    /// Turns the section plane on or off.
    ToggleSection,
    ToggleGroundGrid,
    ToggleScaleBar,
    /// Spreads the parts of the model apart and shows the controls of the
    /// exploded view, or assembles the parts again.
    ToggleExplode,
    /// How far the exploded view spreads the parts (`Scene::set_explode`).
    SetExplode(f32),
    /// Shows only the selected objects, or ends the isolation.
    ToggleIsolate,
    /// Ends the isolation and makes every object visible.
    ShowAll,
    /// Saves the camera as a named view of the document: under this name
    /// (replacing a view of the same name), or the next "View N".
    SaveView(Option<String>),
    /// Moves the camera to the named view at this index.
    GoToView(usize),
    /// Stores the current camera in the named view at this index.
    UpdateView(usize),
    DeleteView(usize),
    TogglePanel(Tab),
    /// Shows this panel (`None`: the focused one) alone in the window, or
    /// brings the whole layout back.
    ToggleMaximize(Option<Tab>),
    ResetLayout,
    /// Opens the search over every command.
    CommandPalette,
    // Tools
    Export(ExportFormat),
    CheckGeometry,
    ToggleInspect,
    /// Opens the screenshot and animation options.
    Screenshot,
    /// Asks where to save a screenshot with the chosen options.
    SaveImage,
    /// Renders a screenshot with the chosen options directly to the clipboard.
    CopyImage,
    /// Asks where to save a turntable animation and starts it.
    ExportTurntable,
    CancelTurntable,
    Upload,
    // Help
    Documentation,
    InstallationGuide,
    CadQueryDocumentation,
    About,
    CheckUpdates,
    /// Opens the page of the newer CQ-editor release found.
    DownloadUpdate,
    // From panels and the view
    /// A line of the script F5 runs.
    GoToLine(usize),
    /// A line of the open document `document` (by generation), the script
    /// of a traceback.
    GoToScriptLine {
        document: u64,
        line: usize,
    },
    /// A line of a file, such as a module in a traceback, opened if needed.
    OpenFileAt {
        path: PathBuf,
        line: usize,
    },
    Select {
        key: Option<NodeKey>,
        topology: Option<TopologySelection>,
        extend: bool,
    },
    SetVisible {
        key: NodeKey,
        visible: bool,
    },
    /// The script line that created an object, or one step of its
    /// construction stack as (frame, item) in the CQ Inspector.
    GoToSource {
        key: NodeKey,
        step: Option<(usize, Option<usize>)>,
    },
    /// The script line that created the current object.
    GoToSelectionSource,
    /// A new value for the parameter `name` assigned on `line` (1-based).
    SetParameter {
        name: String,
        line: usize,
        value: f64,
    },
    /// Restores all modified parameters in the active script to their reference values.
    ResetParameters,
    RemoveSelected,
    ClearObjects,
    InspectEntry {
        frame: usize,
        item: Option<usize>,
    },
    ConsoleSubmit(String),
    /// Tab in the console input.
    ConsoleComplete,
    /// Fresh console namespace.
    ResetConsole,
    StartUpload {
        url: String,
        file_type: String,
    },
}

const fn key(key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::NONE, key)
}

const fn ctrl(key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::COMMAND, key)
}

const fn shift(key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::SHIFT, key)
}

const fn alt(key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::ALT, key)
}

const fn shift_alt(key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::SHIFT.plus(Modifiers::ALT), key)
}

const fn ctrl_shift(key: Key) -> KeyboardShortcut {
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), key)
}

/// The shortcut shown next to an action in menus and tooltips.
pub fn shortcut(action: &Action) -> Option<KeyboardShortcut> {
    shortcuts()
        .into_iter()
        .chain(editor_shortcuts())
        .find(|(a, _)| a == action)
        .map(|(_, shortcut)| shortcut)
}

/// Shortcuts the code editor handles itself, only while it has the
/// keyboard focus: the console and the search fields keep their own undo.
/// Ctrl+Shift+Z also redoes.
pub fn editor_shortcuts() -> Vec<(Action, KeyboardShortcut)> {
    vec![(Action::Undo, ctrl(Key::Z)), (Action::Redo, ctrl(Key::Y))]
}

/// All global shortcuts, most specific first: egui ignores extra Shift/Alt
/// when matching, so Shift+F5 must be checked before F5.
pub fn shortcuts() -> Vec<(Action, KeyboardShortcut)> {
    let mut all = vec![
        (Action::NewFile, ctrl(Key::N)),
        (Action::OpenFile, ctrl(Key::O)),
        (Action::SaveAs, ctrl_shift(Key::S)),
        (Action::Save, ctrl(Key::S)),
        (Action::OpenFolder, ctrl_shift(Key::O)),
        (Action::CloseTab, ctrl(Key::W)),
        (Action::PreviousTab, ctrl_shift(Key::Tab)),
        (Action::NextTab, ctrl(Key::Tab)),
        (Action::PreviousTab, ctrl(Key::PageUp)),
        (Action::NextTab, ctrl(Key::PageDown)),
        (Action::Quit, ctrl(Key::Q)),
        (Action::ToggleComment, ctrl(Key::Slash)),
        (Action::DuplicateLine, ctrl_shift(Key::D)),
        (Action::DuplicateLine, shift_alt(Key::ArrowDown)),
        (Action::MoveLineUp, alt(Key::ArrowUp)),
        (Action::MoveLineDown, alt(Key::ArrowDown)),
        (Action::Autocomplete, ctrl(Key::Space)),
        (Action::Autocomplete, alt(Key::Slash)),
        (Action::Find, ctrl(Key::F)),
        (Action::Replace, ctrl(Key::H)),
        (Action::GoToLinePrompt, ctrl(Key::G)),
        (Action::ShowOutline, alt(Key::O)),
        (Action::GoToDefinition, key(Key::F12)),
        (Action::FindReferences, ctrl_shift(Key::F12)),
        (Action::InsertSelector, ctrl_shift(Key::I)),
        (Action::GoToSelectionSource, ctrl_shift(Key::G)),
        (Action::FindPrevious, alt(Key::F3)),
        (Action::FindNext, key(Key::F3)),
        (Action::Preferences, ctrl(Key::Comma)),
        (Action::CommandPalette, ctrl_shift(Key::P)),
        (Action::ToggleMaximize(None), ctrl_shift(Key::M)),
        (Action::Stop, ctrl_shift(Key::F5)),
        (Action::ToggleDebug, ctrl(Key::F5)),
        (Action::Render, key(Key::F5)),
        (Action::Profile, alt(Key::F5)),
        (Action::Debug(DebugAction::Step), ctrl(Key::F10)),
        (Action::Debug(DebugAction::StepOut), ctrl_shift(Key::F11)),
        (Action::Debug(DebugAction::StepIn), ctrl(Key::F11)),
        (Action::Debug(DebugAction::Continue), ctrl(Key::F12)),
        (Action::Fit, shift(Key::F1)),
        (Action::View(StandardView::Iso), shift(Key::F2)),
        (Action::View(StandardView::Top), shift(Key::F3)),
        (Action::View(StandardView::Bottom), shift(Key::F4)),
        (Action::View(StandardView::Front), shift(Key::F5)),
        (Action::View(StandardView::Back), shift(Key::F6)),
        (Action::View(StandardView::Left), shift(Key::F7)),
        (Action::View(StandardView::Right), shift(Key::F8)),
        (Action::Display(DisplayMode::Wireframe), shift(Key::F9)),
        (Action::Display(DisplayMode::Shaded), shift(Key::F10)),
        (Action::Display(DisplayMode::XRay), shift(Key::F11)),
        (Action::Display(DisplayMode::HiddenLine), shift(Key::F12)),
        (Action::SelectionMode(SelectionMode::Auto), alt(Key::Num1)),
        (Action::SelectionMode(SelectionMode::Object), alt(Key::Num2)),
        (Action::SelectionMode(SelectionMode::Face), alt(Key::Num3)),
        (Action::SelectionMode(SelectionMode::Edge), alt(Key::Num4)),
        (Action::SelectionMode(SelectionMode::Vertex), alt(Key::Num5)),
    ];
    let specificity = |s: &KeyboardShortcut| {
        let m = s.modifiers;
        u8::from(m.command || m.ctrl) + u8::from(m.shift) + u8::from(m.alt)
    };
    // Stable, so equally specific shortcuts keep their listed order.
    all.sort_by_key(|(_, s)| std::cmp::Reverse(specificity(s)));
    all
}

/// Queues the actions whose shortcuts were pressed this frame.
pub fn collect_shortcuts(ctx: &egui::Context, actions: &mut Vec<Action>) {
    ctx.input_mut(|input| {
        for (action, shortcut) in shortcuts() {
            if input.consume_shortcut(&shortcut) {
                actions.push(action);
            }
        }
    });
}

/// Parses a user-configurable shortcut string such as `"1"`, `"F"`, `"Alt+1"`,
/// or `"Ctrl+Shift+F"`.
pub fn parse_shortcut(spec: &str) -> Option<KeyboardShortcut> {
    let parts: Vec<&str> = spec
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    let (&key_part, mod_parts) = parts.split_last()?;
    let mut modifiers = Modifiers::NONE;
    for part in mod_parts {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" | "cmd" | "command" => {
                modifiers = modifiers.plus(Modifiers::COMMAND)
            }
            "shift" => modifiers = modifiers.plus(Modifiers::SHIFT),
            "alt" | "opt" | "option" => modifiers = modifiers.plus(Modifiers::ALT),
            _ => return None,
        }
    }
    let key = parse_key(key_part)?;
    Some(KeyboardShortcut::new(modifiers, key))
}

fn parse_key(token: &str) -> Option<Key> {
    match token.to_ascii_uppercase().as_str() {
        "0" => Some(Key::Num0),
        "1" => Some(Key::Num1),
        "2" => Some(Key::Num2),
        "3" => Some(Key::Num3),
        "4" => Some(Key::Num4),
        "5" => Some(Key::Num5),
        "6" => Some(Key::Num6),
        "7" => Some(Key::Num7),
        "8" => Some(Key::Num8),
        "9" => Some(Key::Num9),
        "A" => Some(Key::A),
        "B" => Some(Key::B),
        "C" => Some(Key::C),
        "D" => Some(Key::D),
        "E" => Some(Key::E),
        "F" => Some(Key::F),
        "G" => Some(Key::G),
        "H" => Some(Key::H),
        "I" => Some(Key::I),
        "J" => Some(Key::J),
        "K" => Some(Key::K),
        "L" => Some(Key::L),
        "M" => Some(Key::M),
        "N" => Some(Key::N),
        "O" => Some(Key::O),
        "P" => Some(Key::P),
        "Q" => Some(Key::Q),
        "R" => Some(Key::R),
        "S" => Some(Key::S),
        "T" => Some(Key::T),
        "U" => Some(Key::U),
        "V" => Some(Key::V),
        "W" => Some(Key::W),
        "X" => Some(Key::X),
        "Y" => Some(Key::Y),
        "Z" => Some(Key::Z),
        "F1" => Some(Key::F1),
        "F2" => Some(Key::F2),
        "F3" => Some(Key::F3),
        "F4" => Some(Key::F4),
        "F5" => Some(Key::F5),
        "F6" => Some(Key::F6),
        "F7" => Some(Key::F7),
        "F8" => Some(Key::F8),
        "F9" => Some(Key::F9),
        "F10" => Some(Key::F10),
        "F11" => Some(Key::F11),
        "F12" => Some(Key::F12),
        _ => None,
    }
}

/// Checks configurable 3D selection mode shortcuts. Plain keys (without Ctrl/Alt)
/// are only consumed when no text editor or input field has keyboard focus.
pub fn collect_selection_shortcuts(
    ctx: &egui::Context,
    config: &SelectionShortcuts,
    actions: &mut Vec<Action>,
) {
    let text_focused = ctx.egui_wants_keyboard_input();
    let mut bindings: Vec<(SelectionMode, KeyboardShortcut)> = SelectionMode::ALL
        .into_iter()
        .filter_map(|mode| {
            let shortcut = parse_shortcut(config.get(mode))?;
            let has_modifier =
                shortcut.modifiers.command || shortcut.modifiers.ctrl || shortcut.modifiers.alt;
            if text_focused && !has_modifier {
                return None;
            }
            Some((mode, shortcut))
        })
        .collect();
    let specificity = |s: &KeyboardShortcut| {
        let m = s.modifiers;
        u8::from(m.command || m.ctrl) + u8::from(m.shift) + u8::from(m.alt)
    };
    bindings.sort_by_key(|(_, s)| std::cmp::Reverse(specificity(s)));

    ctx.input_mut(|input| {
        for (mode, shortcut) in bindings {
            if input.consume_shortcut(&shortcut) {
                actions.push(Action::SelectionMode(mode));
            }
        }
    });
}

#[cfg(test)]
#[path = "../tests/rust/app/test_commands.rs"]
mod tests;
