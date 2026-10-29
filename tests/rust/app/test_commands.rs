use super::*;

#[test]
fn specific_shortcuts_come_first() {
    let all = shortcuts();
    let position = |action: &Action| all.iter().position(|(a, _)| a == action).unwrap();
    assert!(position(&Action::View(StandardView::Front)) < position(&Action::Render));
    assert!(position(&Action::SaveAs) < position(&Action::Save));
    assert!(position(&Action::Stop) < position(&Action::ToggleDebug));
}

#[test]
fn menus_find_their_shortcuts() {
    assert_eq!(shortcut(&Action::Render), Some(key(Key::F5)));
    assert_eq!(shortcut(&Action::Autocomplete), Some(ctrl(Key::Space)));
    assert_eq!(shortcut(&Action::FindPrevious), Some(alt(Key::F3)));
    assert_eq!(shortcut(&Action::FindNext), Some(key(Key::F3)));
    assert_eq!(shortcut(&Action::Upload), None);
}

#[test]
fn selector_commands_have_menu_entries_and_a_free_shortcut() {
    assert_eq!(shortcut(&Action::InsertSelector), Some(ctrl_shift(Key::I)));
    // Copy has none: Ctrl+Shift+C would also copy the editor's text.
    assert_eq!(shortcut(&Action::CopySelector), None);
    let taken = shortcuts()
        .iter()
        .filter(|(_, binding)| *binding == ctrl_shift(Key::I))
        .count();
    assert_eq!(taken, 1, "Ctrl+Shift+I is bound once");
}

#[test]
fn legacy_shortcuts_are_preserved() {
    let all = shortcuts();
    let has = |action: Action, binding: KeyboardShortcut| {
        assert!(
            all.contains(&(action.clone(), binding)),
            "missing legacy shortcut for {action:?}"
        );
    };

    has(Action::NewFile, ctrl(Key::N));
    has(Action::OpenFile, ctrl(Key::O));
    has(Action::Save, ctrl(Key::S));
    has(Action::SaveAs, ctrl_shift(Key::S));
    has(Action::Quit, ctrl(Key::Q));
    has(Action::ToggleComment, ctrl(Key::Slash));
    has(Action::Autocomplete, alt(Key::Slash));
    has(Action::Find, ctrl(Key::F));

    has(Action::Render, key(Key::F5));
    has(Action::ToggleDebug, ctrl(Key::F5));
    has(Action::Debug(DebugAction::Step), ctrl(Key::F10));
    has(Action::Debug(DebugAction::StepIn), ctrl(Key::F11));
    has(Action::Debug(DebugAction::Continue), ctrl(Key::F12));

    has(Action::Fit, shift(Key::F1));
    for (view, key) in [
        (StandardView::Iso, Key::F2),
        (StandardView::Top, Key::F3),
        (StandardView::Bottom, Key::F4),
        (StandardView::Front, Key::F5),
        (StandardView::Back, Key::F6),
        (StandardView::Left, Key::F7),
        (StandardView::Right, Key::F8),
    ] {
        has(Action::View(view), shift(key));
    }
    has(Action::Display(DisplayMode::Wireframe), shift(Key::F9));
    has(Action::Display(DisplayMode::Shaded), shift(Key::F10));
}

#[test]
fn the_new_display_modes_follow_on_the_next_function_keys() {
    let has = |action: Action, expected| assert_eq!(shortcut(&action), Some(expected));
    has(Action::Display(DisplayMode::XRay), shift(Key::F11));
    has(Action::Display(DisplayMode::HiddenLine), shift(Key::F12));
    // Stepping out keeps Ctrl+Shift+F11: the more specific match wins.
    let order: Vec<_> = shortcuts().into_iter().map(|(action, _)| action).collect();
    let position = |action: &Action| order.iter().position(|a| a == action).unwrap();
    assert!(
        position(&Action::Debug(DebugAction::StepOut))
            < position(&Action::Display(DisplayMode::XRay))
    );
}

#[test]
fn go_to_source_has_its_own_shortcut() {
    assert_eq!(
        shortcut(&Action::GoToSelectionSource),
        Some(ctrl_shift(Key::G))
    );
    let taken = shortcuts()
        .iter()
        .filter(|(_, binding)| *binding == ctrl_shift(Key::G))
        .count();
    assert_eq!(taken, 1);
}

#[test]
fn profiling_has_its_own_shortcut_next_to_render() {
    assert_eq!(shortcut(&Action::Profile), Some(alt(Key::F5)));
    // Alt+F5 is matched before the plain F5 of Render.
    let order: Vec<_> = shortcuts().into_iter().map(|(action, _)| action).collect();
    let position = |action: &Action| order.iter().position(|a| a == action).unwrap();
    assert!(position(&Action::Profile) < position(&Action::Render));
}

#[test]
fn tabs_have_the_usual_shortcuts_and_none_is_bound_twice() {
    assert_eq!(shortcut(&Action::CloseTab), Some(ctrl(Key::W)));
    assert_eq!(shortcut(&Action::NextTab), Some(ctrl(Key::Tab)));
    assert_eq!(shortcut(&Action::PreviousTab), Some(ctrl_shift(Key::Tab)));
    assert_eq!(shortcut(&Action::OpenFolder), Some(ctrl_shift(Key::O)));
    let all = shortcuts();
    let position = |action: &Action| all.iter().position(|(a, _)| a == action).unwrap();
    assert!(position(&Action::PreviousTab) < position(&Action::NextTab));
    // Ctrl+PageUp and Ctrl+PageDown switch tabs too.
    assert!(all.contains(&(Action::NextTab, ctrl(Key::PageDown))));
    assert!(all.contains(&(Action::PreviousTab, ctrl(Key::PageUp))));

    let mut bindings: Vec<_> = all.iter().map(|(_, binding)| *binding).collect();
    let count = bindings.len();
    bindings.sort_by_key(|binding| format!("{binding:?}"));
    bindings.dedup();
    assert_eq!(bindings.len(), count, "a key combination is bound twice");
}

#[test]
fn selection_modes_have_default_and_configurable_shortcuts() {
    for (mode, digit) in [
        (SelectionMode::Auto, Key::Num1),
        (SelectionMode::Object, Key::Num2),
        (SelectionMode::Face, Key::Num3),
        (SelectionMode::Edge, Key::Num4),
        (SelectionMode::Vertex, Key::Num5),
    ] {
        assert_eq!(shortcut(&Action::SelectionMode(mode)), Some(alt(digit)));
    }
    assert_eq!(parse_shortcut("3"), Some(key(Key::Num3)));
    assert_eq!(parse_shortcut("Alt+4"), Some(alt(Key::Num4)));
    assert_eq!(parse_shortcut("Ctrl+Shift+F"), Some(ctrl_shift(Key::F)));
    assert_eq!(parse_shortcut(""), None);
}
