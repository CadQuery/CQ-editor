use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

const MATRIX: &str = include_str!("../../../docs/development/compatibility.md");

const REQUIRED_CONTRACTS: &[&str] = &[
    "file.lifecycle",
    "file.autoreload",
    "file.recovery",
    "editor.syntax",
    "editor.editing",
    "editor.search",
    "editor.completion",
    "script.examples",
    "script.show-object",
    "script.discovery",
    "script.logging",
    "script.execution-settings",
    "debugger.controls",
    "debugger.breakpoints",
    "debugger.errors",
    "console.state",
    "console.rich-output",
    "inspector.workplane",
    "objects.tree",
    "objects.properties",
    "objects.assemblies",
    "export.step-stl",
    "tools.upload",
    "tools.update",
    "viewer.standard-views",
    "viewer.display-modes",
    "viewer.projection",
    "viewer.mouse",
    "viewer.navigation-cube",
    "viewer.selection",
    "viewer.screenshot",
    "viewer.stereo",
    "panels.docking",
    "preferences.persistence",
    "preferences.defaults",
    "shortcuts.legacy",
    "behavior.rand-color",
    "ui.theme",
    "packaging.distribution",
    "worker.isolation",
];

#[test]
fn compatibility_matrix_is_complete_and_evidenced() {
    let mut rows = BTreeMap::new();
    for line in MATRIX.lines() {
        let cells: Vec<_> = line.split('|').map(str::trim).collect();
        if cells.len() != 6 || !cells[1].starts_with('`') || !cells[1].ends_with('`') {
            continue;
        }
        let id = cells[1].trim_matches('`').to_owned();
        assert!(
            rows.insert(id.clone(), (cells[3], cells[4])).is_none(),
            "duplicate compatibility row {id}"
        );
    }

    let expected: BTreeSet<_> = REQUIRED_CONTRACTS.iter().copied().collect();
    let actual: BTreeSet<_> = rows.keys().map(String::as_str).collect();
    assert_eq!(actual, expected, "compatibility inventory changed silently");

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (id, (status, evidence)) in rows {
        match status {
            "Verified" | "Enhanced" => {
                let reference = first_code_span(evidence)
                    .unwrap_or_else(|| panic!("{id} has no automated evidence"));
                let (relative_path, test_name) = reference
                    .rsplit_once("::")
                    .unwrap_or_else(|| panic!("invalid evidence for {id}: {reference}"));
                assert!(
                    relative_path.starts_with("tests/"),
                    "{id} evidence must name a test file"
                );
                let source_path = root.join(relative_path);
                let source = fs::read_to_string(&source_path).unwrap_or_else(|error| {
                    panic!(
                        "cannot read evidence for {id} at {}: {error}",
                        source_path.display()
                    )
                });
                assert!(
                    source.contains(&format!("{test_name}(")),
                    "evidence function {test_name} for {id} is missing from {relative_path}"
                );
            }
            "Changed" | "Gap" => assert!(
                evidence.starts_with("Decision:"),
                "{id} needs an explicit decision"
            ),
            "Unverified" => assert!(
                evidence.starts_with("Follow-up:"),
                "{id} needs an explicit verification follow-up"
            ),
            other => panic!("unknown compatibility state {other} for {id}"),
        }
    }

    assert!(MATRIX.contains("commit [`71903d5`]"));
    assert!(MATRIX.contains("## Change policy"));
}

fn first_code_span(text: &str) -> Option<&str> {
    let start = text.find('`')? + 1;
    let end = start + text[start..].find('`')?;
    Some(&text[start..end])
}
