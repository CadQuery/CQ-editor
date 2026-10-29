use super::*;

fn labels(found: &[Command]) -> Vec<&str> {
    found.iter().map(|command| command.label.as_str()).collect()
}

#[test]
fn every_command_with_a_shortcut_can_be_found() {
    let listed = menu_commands();
    for (action, _) in commands::shortcuts() {
        assert!(
            listed.iter().any(|command| command.action == action),
            "{action:?} is missing from the palette"
        );
    }
}

#[test]
fn queries_match_letters_in_order_and_ignore_case() {
    assert!(score("rnd", "Run: Render").is_some());
    assert!(score("RENDER", "Run: Render").is_some());
    assert!(score("rendr", "Run: Render").is_some());
    assert_eq!(score("xyz", "Run: Render"), None);
    // Out of order is not a match.
    assert_eq!(score("redner", "Run: Render"), None);
    // An empty query lists everything.
    assert_eq!(score("  ", "anything"), Some(0));
    // A word start beats letters scattered inside words.
    assert!(score("st", "Run: Stop") > score("st", "View: Show Last"));
}

#[test]
fn whole_words_and_word_starts_come_first() {
    let found = matches(menu_commands(), "step");
    assert_eq!(labels(&found)[..3], ["Step", "Step In", "Step Out"]);

    let found = matches(menu_commands(), "save as");
    assert_eq!(found[0].label, "Save As…");

    // The category counts too: "run stop" finds Run: Stop.
    let found = matches(menu_commands(), "run stop");
    assert_eq!(found[0].action, Action::Stop);

    let found = matches(menu_commands(), "exp stl");
    assert_eq!(found[0].action, Action::Export(ExportFormat::Stl));

    let found = matches(menu_commands(), "max");
    assert_eq!(found[0].action, Action::ToggleMaximize(None));
}

#[test]
fn only_commands_usable_now_are_offered() {
    let state = crate::app::tests::test_state("x = 1\n");
    let offered = commands(&state);
    let has = |action: &Action| offered.iter().any(|command| command.action == *action);
    assert!(has(&Action::Render));
    assert!(has(&Action::OpenFolder));
    assert!(!has(&Action::Debug(DebugAction::Step)), "not paused");
    assert!(
        !has(&Action::Export(ExportFormat::Step)),
        "nothing to export"
    );
    assert!(!has(&Action::CloseFolder), "no folder");
    assert!(!has(&Action::CommandPalette), "already open");
}

#[test]
fn favorite_commands_appear_first_and_persist_in_settings() {
    let mut settings = crate::settings::Settings::default();
    let id = "Tools: Check Geometry";
    assert!(!settings.is_favorite_command(id));
    assert!(settings.toggle_favorite_command(id));
    assert!(settings.is_favorite_command(id));

    let ordered = matches_with_favorites(menu_commands(), "", &settings.favorite_commands);
    assert_eq!(ordered[0].id(), id);
    assert_eq!(ordered[0].action, Action::CheckGeometry);

    assert!(!settings.toggle_favorite_command(id));
    assert!(!settings.is_favorite_command(id));
}
