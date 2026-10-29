use super::*;

fn total_width() -> f32 {
    Group::ALL.iter().map(|group| group.width()).sum::<f32>()
        + SEPARATOR_WIDTH * (Group::ALL.len() - 1) as f32
}

#[test]
fn every_group_shows_when_the_window_is_wide_enough() {
    assert_eq!(fitting_groups(total_width()), Group::ALL.len());
    assert_eq!(fitting_groups(4000.0), Group::ALL.len());
}

#[test]
fn groups_that_do_not_fit_go_to_the_overflow_menu_from_the_last() {
    // The narrowest window CQ-editor allows (760 points).
    let shown = fitting_groups(760.0);
    assert!(shown > 0 && shown < Group::ALL.len());
    let used: f32 = Group::ALL[..shown]
        .iter()
        .map(|group| group.width())
        .sum::<f32>()
        + SEPARATOR_WIDTH * shown as f32
        + BUTTON_SIZE.x;
    assert!(used <= 760.0, "the shown groups and » fit: {used}");
    // The file and run commands stay in view.
    assert!(shown >= 3, "{shown}");

    // A point short of the whole toolbar moves only the last group.
    assert_eq!(fitting_groups(total_width() - 1.0), Group::ALL.len() - 1);
    assert_eq!(fitting_groups(0.0), 0);
}
