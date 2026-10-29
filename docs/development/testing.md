# Testing

## Running the tests

From the repository root, with supported Python 3.11–3.14, CadQuery, pytest (and optionally jedi)
installed in the active Python environment:

```sh
cargo test --locked                          # Rust tests
PYTHONPATH=worker python -m pytest tests/worker   # worker tests
```

In PowerShell, set the path first:

```powershell
$env:PYTHONPATH = 'worker'
python -m pytest tests/worker
```

The network tests use local fakes. The process tests start a real worker but
never contact external services.

## Layout

The test folders follow the parts of the application they protect:

| Folder | Coverage |
| --- | --- |
| `tests/packaging/` | Version synchronization, bundled dependency pins, native shortcuts and the release workflow contract |
| `tests/rust/app/` | Commands, the run and request state machine, editor tabs and projects, preferences, file watching, logging, embedded files and the command line |
| `tests/rust/editor/` | Document persistence, recovery, text editing and highlighting |
| `tests/rust/panels/` | Console behavior and the Objects panel layout |
| `tests/rust/protocol/` | JSON messages, binary mesh decoding and interpreter discovery |
| `tests/fixtures/protocol/` | Golden worker messages read by both Rust and Python, including references, non-finite data and oversized output |
| `tests/rust/scene/` | Scene tree, visibility, selection, colors and bounds |
| `tests/rust/viewer/` | Camera, navigation overlays, picking, section planes, WGSL validation and GPU rendering |
| `tests/worker/execution/` | Script environment, debugger, console and completion |
| `tests/worker/geometry/` | Scene building, display options, colors, registry, glTF reading, tessellation and inspection |
| `tests/worker/protocol/` | Request handling, transport, uploads, releases and real worker processes |

The shaders are parsed and validated with naga, wgpu's WGSL front end, and
their uniform layouts are compared with the Rust structs, so these tests need
no GPU. The rendering tests in `test_renderer.rs` draw offscreen on whatever
adapter wgpu finds, software adapters included, one test at a time on a
shared device, and pass without checking anything (with a message on
stderr) on a machine that has none. Set `CQ_EDITOR_REQUIRE_GPU=1` to make a
missing adapter fail instead (CI does on Windows), and
`WGPU_ADAPTER_NAME` (part of a name) or `WGPU_BACKEND` to choose one, for
example `WGPU_ADAPTER_NAME="Microsoft Basic Render Driver"` for the
software adapter CI uses on Windows. On Linux, CI renders with Mesa's
llvmpipe through wgpu's OpenGL backend; `WGPU_BACKEND=gl` reproduces that
backend's limits on any machine.

Rust tests live in `tests/rust/` but are **included from the source module
they test** with `#[cfg(test)] #[path = "…"] mod tests;`. They can therefore
exercise private items without making them public. Python tests share the
protocol fixtures of `tests/worker/conftest.py`. Both suites also read
`tests/fixtures/protocol/messages.json`: the Python writer must emit strict
JSON, and the Rust reader must accept the same mesh parts and references
within its message limits. `tests/rust/app/` draws a modal and a restored
dock with egui's headless `Context::run_ui`, then checks keyboard actions and
visible panels without opening a native window.

## Continuous integration

`.github/workflows/ci.yml` runs on every push to `master` and on pull
requests:

| Job | Steps |
| --- | --- |
| `packaging` (Ubuntu 22.04) | Installer metadata, version and release-workflow contract tests |
| `rust` (Ubuntu 22.04, Windows, macOS) | `cargo fmt --all -- --check` · `cargo clippy --locked --all-targets --all-features -- -D warnings` · `cargo test --locked` |
| `worker` released matrix (Ubuntu 22.04) | Python 3.11/CadQuery 2.7/OCP 7.8 and Python 3.11–3.14/CadQuery 2.8/OCP 7.9 · package check · Black · Ruff · mypy across all worker modules · full pytest suite with branch coverage at least 85% |
| `worker` development probe (Ubuntu 22.04) | Python 3.12 · current `CadQuery/master` · OCP 8.0 · package check · Black · Ruff · mypy · full pytest suite with branch coverage; early-warning lane, allowed to fail while upstream moves |

Run the same checks locally before opening a pull request:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked
black --check worker tests/worker tests/packaging packaging/constructor/prepare_macos_icon.py
ruff check --isolated --select E4,E7,E9,F worker tests/worker tests/packaging packaging/constructor/prepare_macos_icon.py
mypy --config-file worker/pyproject.toml worker/cq_worker
python -m unittest discover -s tests/packaging -p "test_*.py"
PYTHONPATH=worker python -m coverage run --branch --source worker/cq_worker -m pytest tests/worker
python -m coverage report --show-missing --fail-under=85
```

The CI run is the authoritative status. Do not hand-maintain test totals here:
parameterized compatibility cases make the number change often. CI installs
the latest `black`; use the version reported by the failing job if local and
CI formatting differ.

See [CadQuery and OCP compatibility](dependency-compatibility.md) for the
support policy and exact dependency pins. Pytest's header records Python,
CadQuery, OCP, platform and interpreter for every saved result.

## Original compatibility gate

The [compatibility matrix](compatibility.md) is part of the test suite, not
just a checklist. `compatibility_matrix_is_complete_and_evidenced` contains
the stable legacy contract ids and fails when:

* an inventoried capability silently disappears from the matrix;
* a Verified or Enhanced row does not name an existing test function;
* a Changed or Gap row has no explicit decision; or
* an Unverified GUI interaction has no follow-up.

`legacy_shortcuts_are_preserved` checks every original shortcut,
`legacy_mouse_gestures_keep_their_navigation_modes` checks the original mouse
bindings, and `test_examples.py` executes every example project, including
the eight (`01` to `08`) that are unchanged from the pre-rewrite revision. This gate checks inventory and
evidence; it deliberately does not claim that documented gaps are complete.

## Benchmarks

Benchmarks are `#[ignore]`d tests, so `cargo test` skips them. Run them in
release mode, where the timings are meaningful:

```sh
cargo test --release pick_latency -- --ignored --nocapture
```

`pick_latency_on_growing_assemblies` sweeps the pointer across assemblies
of 64 to 16,384 instanced parts and prints the median, 95th percentile and
maximum pick latency without and with the scene index, plus the index
build time. Its first line records CQ-editor version, operating system,
architecture, build profile and logical CPU count so results are comparable.

## Writing tests

* **Rust:** put the test next to its siblings in `tests/rust/<area>/` and
  include it from the module, as the existing files do. Keep tests free of
  a GPU or a window: test the camera math, picking, scene and text
  operations directly.
* **Worker:** prefer calling `Server.dispatch` or the module functions with
  small CadQuery shapes. Use the fixtures in `conftest.py` for protocol
  round trips. Tests that start a real `python -m cq_worker` process belong
  in `tests/worker/protocol/`.
* **Regression tests first:** a bug fix should come with a test that fails
  without it.
