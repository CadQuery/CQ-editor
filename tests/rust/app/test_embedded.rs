use super::*;

#[test]
fn titles_drop_numbering_and_capitalize() {
    assert_eq!(example_title("01_hello_box.py"), "Hello Box");
    assert_eq!(
        example_title("selectors_and_fillets.py"),
        "Selectors And Fillets"
    );
    assert_eq!(example_title("loft.py"), "Loft");
}

#[test]
fn everything_needed_is_embedded() {
    let names: Vec<_> = WORKER_FILES.iter().map(|(name, _)| *name).collect();
    assert!(names.contains(&"cq_worker/__main__.py"));
    assert!(names.contains(&"cq_worker/server.py"));
    assert!(!examples().is_empty());
}

#[test]
fn extracting_a_new_worker_removes_the_other_versions() {
    let cache =
        std::env::temp_dir().join(format!("cq-editor-embedded-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cache);
    let old = extract(&cache, &[("cq_worker/__init__.py", "old = True\n")]).unwrap();
    std::fs::create_dir_all(cache.join("worker-interrupted")).unwrap();
    std::fs::write(cache.join("settings.ron"), "kept").unwrap();

    let new = extract(&cache, &[("cq_worker/__init__.py", "new = True\n")]).unwrap();

    assert_ne!(old, new);
    assert!(!old.exists(), "the previous version is gone");
    assert!(!cache.join("worker-interrupted").exists());
    assert!(cache.join("settings.ron").exists(), "other data stays");
    assert_eq!(
        std::fs::read_to_string(new.join("cq_worker/__init__.py")).unwrap(),
        "new = True\n"
    );
    // Starting the same version again changes nothing.
    assert_eq!(
        extract(&cache, &[("cq_worker/__init__.py", "new = True\n")]).unwrap(),
        new
    );
    std::fs::remove_dir_all(cache).unwrap();
}
