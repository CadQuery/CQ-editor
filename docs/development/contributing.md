# Contributing

Contributions of all sizes are welcome: bug reports, documentation fixes,
examples, features. For larger changes, open an issue or a
[discussion](https://github.com/igors93/CQ-editor/discussions) first, so
the direction can be agreed on before you invest time. The
[roadmap](../roadmap.md) lists known bugs and planned work.

## Workflow

1. Fork the repository and create a branch from `master`.
2. Build and run the editor ([Building](building.md)).
3. Make your change, with tests ([Testing](testing.md)).
4. Run the CI checks locally:

   ```sh
   cargo fmt --all -- --check
   cargo clippy --locked --all-targets --all-features -- -D warnings
   cargo test --locked
   black --check worker tests/worker tests/packaging packaging/constructor/prepare_macos_icon.py
   ruff check --isolated --select E4,E7,E9,F worker tests/worker tests/packaging packaging/constructor/prepare_macos_icon.py
   python -m unittest discover -s tests/packaging -p "test_*.py"
   PYTHONPATH=worker python -m coverage run --branch --source worker/cq_worker -m pytest tests/worker
   python -m coverage report --show-missing --fail-under=85
   ```

5. Update the documentation in `docs/` if behavior changed, and add a line
   to [the changelog](../changelog.md) under *Unreleased*.
6. Open a pull request describing what changed and why.

## Code style

* **Rust:** `cargo fmt`, no clippy warnings, edition 2024.
* **Python:** `black`, type hints, `from __future__ import annotations`, and
  compatibility with the supported Python/CadQuery/OCP matrix (the worker
  runs in the user's environment).
* **Comments and docs in English.** Comments explain *why*, not what the
  code already says. Every module starts with a short doc comment saying
  what it is for.
* **Keep the worker dependency-free** beyond CadQuery, numpy and the
  standard library (`jedi` is optional). It runs inside environments we do
  not control.

## Where to make common changes

| Change | Where |
| --- | --- |
| A new command (menu item, toolbar button, shortcut) | Add a variant to `Action` in `src/commands.rs` (and a shortcut in `shortcuts()`), handle it in `AppState::execute` in `src/app/mod.rs` (dock layout actions go to `CqEditorApp::execute`) and put the work in the `src/app/` module of its concern, then add the menu item in `src/ui/menu_bar.rs` and/or the button in `src/ui/toolbar.rs` |
| A new preference | A field with a default in `src/settings.rs` (keep `#[serde(default)]` so old `app.ron` files still load), a row in `src/dialogs/preferences.rs`, a line in [Preferences](../user-guide/preferences.md) |
| A new worker request | A `Request` and a `Message` variant in `src/worker/protocol.rs`, a handler in `Server._handlers` (`worker/cq_worker/server.py`), and a row in [Worker protocol](protocol.md). Bump `PROTOCOL_VERSION` on both sides if old workers would misbehave |
| A new `show_object` option | `DisplayOptions` in `worker/cq_worker/scene.py`, `SceneNode` in `protocol.rs`, the renderer, and [Writing scripts](../user-guide/scripting.md) |
| A new panel | A `Tab` in `src/panels/mod.rs` with its default position, a module in `src/panels/` |
| A new example | A commented `NN_name.py` in `examples/`. It appears in *File > Examples* automatically |
| How runs, queued renders, stale results or pending requests behave | `Execution` in `src/app/execution.rs`, a pure state machine; add the transition to `tests/rust/app/test_execution.rs` |
| A change to how meshes are produced | `worker/cq_worker/tessellation.py`; bump `MESH_FORMAT` when the same shape and tolerances would mesh differently, so the GUI never reuses a mesh of the previous format |
| A new export format | `ExportFormat` in `protocol.rs` and `FORMATS` / `export()` in `worker/cq_worker/exporting.py` |

## Principles to keep

These are what make CQ-editor CQ-editor (see [Architecture](architecture.md#design-principles)
and [Roadmap](../roadmap.md#guiding-principles)):

* the **script stays the source of truth**. A GUI feature may generate or
  insert code, but it must never hide the model in a GUI-only state;
* scripts written for the original CQ-editor must **keep working**;
* the interface must **never block** on Python;
* the editor must **never lose user work**.

## Reporting bugs

See [Reporting a bug](../../README.md#reporting-a-bug) in the README. Please
include the lines from the Log panel and the versions from *Help > About*.
