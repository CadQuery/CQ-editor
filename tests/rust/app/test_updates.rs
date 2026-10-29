use super::*;

fn release(tag: &str, url: &str) -> Release {
    Release {
        tag: tag.to_owned(),
        name: tag.to_owned(),
        url: url.to_owned(),
    }
}

#[test]
fn versions_compare_by_their_numbers() {
    assert_eq!(version_key("v1.2.3-rc1"), (1, 2, 3));
    assert_eq!(version_key("0.8"), (0, 8, 0));
    assert_eq!(version_key("release"), (0, 0, 0));
    assert!(is_newer("v0.10.1", "0.9.9"), "numbers, not text");
    assert!(!is_newer("v0.8.0", "0.8.0"));
    assert!(!is_newer("0.7.9", "0.8.0"));
}

#[test]
fn the_check_runs_once_a_day_unless_turned_off() {
    let mut general = General::default();
    assert!(general.check_updates, "on by default");
    let now = 1_800_000_000;
    assert!(check_due(&general, now), "never checked");
    general.last_update_check = now - CHECK_INTERVAL + 60;
    assert!(!check_due(&general, now));
    general.last_update_check = now - CHECK_INTERVAL;
    assert!(check_due(&general, now));
    general.last_update_check = now + 3600;
    assert!(!check_due(&general, now), "a clock set back waits");
    general.last_update_check = 0;
    general.check_updates = false;
    assert!(!check_due(&general, now));
}

#[test]
fn only_a_newer_release_with_a_github_page_is_offered() {
    let mut general = General::default();
    assert!(available(&general).is_none());
    let page = "https://github.com/igors93/CQ-editor/releases/tag/v99.0.0";
    general.newer_release = Some(release("v99.0.0", page));
    assert_eq!(available(&general).unwrap().url, page);
    // Updated since it was found.
    general.newer_release = Some(release(VERSION, page));
    assert!(available(&general).is_none());
    general.newer_release = Some(release("v99.0.0", "https://example.com/setup.exe"));
    assert!(
        available(&general).is_none(),
        "pages elsewhere are not opened"
    );
}
