use super::*;

#[test]
fn parameters_are_parsed_once_per_text() {
    let mut state = ParametersState::default();
    let text = "a = 1\nb = 2.5\n";
    assert_eq!(state.of(text).len(), 2);
    let first = state.parsed.as_ref().map(|(hash, _)| *hash);
    state.of(text);
    assert_eq!(state.parsed.as_ref().map(|(hash, _)| *hash), first);
    assert_eq!(state.of("a = 1\n").len(), 1);
}

#[test]
fn sliders_keep_their_range_during_a_drag() {
    let mut state = ParametersState::default();
    let width = |text: &str| crate::parameters::parameters(text).remove(0);

    assert_eq!(state.slider_range(&width("width = 40")), 0.0..=80.0);
    // Dragging changes the value, not the range under the pointer.
    assert_eq!(state.slider_range(&width("width = 70")), 0.0..=80.0);
    // A value typed outside widens it.
    assert_eq!(state.slider_range(&width("width = 120")), 0.0..=120.0);
    // A commented range wins.
    assert_eq!(
        state.slider_range(&width("width = 40 # [10, 50]")),
        10.0..=50.0
    );
}

#[test]
fn automatic_ranges_suit_the_sign_and_size() {
    assert_eq!(automatic_range(2.5, false), 0.0..=10.0);
    assert_eq!(automatic_range(-30.0, false), -60.0..=0.0);
    assert_eq!(automatic_range(0.0, true), -5.0..=5.0);
    assert_eq!(automatic_range(7.0, true), 0.0..=14.0);
}

#[test]
fn modified_parameters_are_tracked_against_the_saved_baseline() {
    let mut state = ParametersState::default();
    let baseline = "width = 40\nheight = 20\n";
    let edited = "width = 55\nheight = 20\ndepth = 10\n";
    let current = crate::parameters::parameters(edited);

    state.update_references(baseline, &current);
    assert_eq!(state.reference_value("width"), Some(40.0));
    assert_eq!(state.reference_value("height"), Some(20.0));
    assert_eq!(state.reference_value("depth"), Some(10.0));
    assert!(state.is_modified(&current[0]));
    assert!(!state.is_modified(&current[1]));
    assert!(!state.is_modified(&current[2]));
    assert!(state.any_modified(&current));
}
