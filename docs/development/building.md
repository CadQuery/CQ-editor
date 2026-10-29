# Building

## Requirements

* **Rust 1.88 or newer** (edition 2024), installed with
  [rustup](https://rustup.rs).
* Python 3.11–3.14 with a matching supported CadQuery/OCP environment, to *run* the editor and the worker
  tests (see [Installation](../getting-started/installation.md)). Python is
  not needed to compile.

```sh
cargo build             # debug build: target/debug/cq-editor
cargo build --release   # release build: target/release/cq-editor
cargo run -- examples/01_hello_box.py
```

### Debug and release builds

| | Debug | Release |
| --- | --- | --- |
| Worker sources | Loaded from `worker/` in the source tree. Python edits apply on the next worker restart (Run > Restart Python) | Embedded in the executable and extracted to the data folder on first use |
| Console window on Windows | Yes (logs are visible) | No (`windows_subsystem = "windows"`). `--help` and command-line errors are written to the console of the shell that started it, or shown in a message box |
| Optimization | Dependencies are optimized (`opt-level = 2`), the crate itself is not, and only line tables are kept for debugging | Fat LTO with one codegen unit, stripped |
| `.venv` discovery | Also looks in the source tree | Next to the executable and in the working directory |

`CQ_EDITOR_WORKER_DIR=/path/to/dir` makes any build load the `cq_worker`
package from `dir` instead.

`build.rs` embeds every `.py` file of `worker/cq_worker/` and `examples/`.
Adding a worker module or an example therefore needs no code change: it is
picked up on the next build.

## Linux

Install the development packages for keyboard handling and the GTK file
dialogs (the same ones CI uses on Ubuntu 22.04):

```sh
sudo apt-get install -y libxkbcommon-dev libgtk-3-dev
```

## macOS

Install the Xcode command line tools (`xcode-select --install`). No other
dependency is needed.

## Windows

Two Rust toolchains work.

### MSVC (recommended for most people)

Install the *Desktop development with C++* workload of Visual Studio (or the
Build Tools), then:

```powershell
rustup default stable-x86_64-pc-windows-msvc
cargo build --release
```

Nothing else is needed.

### GNU (no Visual Studio)

The `x86_64-pc-windows-gnu` toolchain avoids Visual Studio, but its minimal
self-contained MinGW environment lacks **`as` and `dlltool`** from MinGW-w64
binutils, used by the `windows` crates to build import libraries. Install
binutils, for example from [WinLibs](https://winlibs.com) or MSYS2 (`pacman
-S mingw-w64-x86_64-binutils`), and put its `bin` folder on `PATH`.
Otherwise the build fails with *dlltool could not create import library*.

Then build from a shell where binutils is on `PATH`:

```sh
export PATH="$HOME/.cargo/bin:/path/to/mingw/bin:$PATH"
cargo build --release
```

The `.exe` gets its icon and version information (shown by Explorer) from a
resource compiled by `build.rs` with `windres`, which also needs `gcc` as its
preprocessor: both come with a full MinGW-w64 installation (MSYS2: `pacman
-S mingw-w64-x86_64-gcc`). Without them the build still succeeds, with a
warning and an executable without that information. The MSVC toolchain uses
`rc.exe` from the Windows SDK instead. Release builds in CI set
`CQ_EDITOR_REQUIRE_RESOURCES=1`, which makes a missing resource compiler an
error.

Links are opened with the `open` crate rather than eframe's `links` feature,
so the GNU toolchain no longer needs a hand-made `libshlwapi.a`.

Setting `CARGO_INCREMENTAL=0` reduces the size of `target/` considerably,
which helps on small disks.

## Release builds in CI

`.github/workflows/release.yml` runs for a version tag (or manually without
publishing). It builds the native Rust executable and then uses constructor
3.17.3 to create four self-contained distributions:

* Windows x86_64 `.exe`;
* Linux x86_64 self-extracting `.sh`;
* macOS Apple Silicon `.pkg`;
* macOS Intel `.pkg`.

`packaging/constructor/construct.yaml` pins Python, CadQuery, OCP, Jedi and
menuinst. The Rust executable is copied into the same private prefix and
`src/worker/launch.rs` discovers that bundled Python before global or active
environments. Explicit user choices still win.

That runtime is most of an installer: Qt 6 (which VTK links), VTK, OCCT,
LLVM (for Numba) and Python, against about 20 MB for the editor. Its
`exclude` list leaves out the Boost, Eigen and TBB development files that
the `vtk` metapackage brings for compiling against VTK, about 17,000 files
and 230 MB once installed, which nothing loads at run time. Constructor
fails if a listed package is no longer in the solved environment, so check
the list when a pin changes.

The Windows job signs the native executable and the finished installer with
Authenticode and an RFC 3161 timestamp. The macOS jobs sign the native editor
with Developer ID Application; constructor signs the embedded bootstrap and
package with the Application and Installer identities. Apple notarization must
accept each package, and the ticket is stapled and verified before upload.
Missing credentials or a failed signature stops the release. Linux continues
to publish its checksum without platform code signing.

Configure these GitHub Actions repository secrets before pushing a release tag:

| Platform | Secret | Value |
| --- | --- | --- |
| Windows | `WINDOWS_SIGNING_PFX_BASE64` | Base64 of the Authenticode `.pfx` including its private key |
| Windows | `WINDOWS_SIGNING_PFX_PASSWORD` | Password for that `.pfx` |
| macOS | `MACOS_APPLICATION_P12_BASE64`, `MACOS_APPLICATION_P12_PASSWORD` | Base64 of the Developer ID Application `.p12` and its password |
| macOS | `MACOS_INSTALLER_P12_BASE64`, `MACOS_INSTALLER_P12_PASSWORD` | Base64 of the Developer ID Installer `.p12` and its password |
| macOS | `MACOS_APPLICATION_IDENTITY`, `MACOS_INSTALLER_IDENTITY` | Full identity names, including team ID, matching the certificates |
| macOS | `MACOS_NOTARY_KEY_P8_BASE64`, `MACOS_NOTARY_KEY_ID`, `MACOS_NOTARY_ISSUER_ID` | Base64 App Store Connect API private key, key ID and issuer ID for `notarytool` |

The signing certificates are written only to the hosted runner's temporary
directory. The Windows PFX is deleted after each signature, and the macOS
temporary keychain is deleted at the end of the job. The workflow never puts
certificates in the repository or release artifacts. Constructor's
[signing options](https://conda.github.io/constructor/howto/#signing-and-notarization)
are enabled only when the macOS identities are supplied; local builds without
credentials are still possible but must not be published as release artifacts.

Each matrix job installs its finished artifact, imports the bundled CAD
runtime and starts the installed executable with `--help`. Publication waits
for all four smoke tests, generates `SHA256SUMS.txt`, creates the GitHub
release and attaches the installers plus constructor's dependency lockfiles.
The tag must exactly equal the Cargo package version, for example `0.9.0`.

To build on one platform manually, first create `target/release/cq-editor`
(or `.exe`), install constructor 3.17.3 from conda-forge and run:

```sh
mkdir dist
cd dist
constructor ../packaging/constructor
```

macOS additionally needs the `.icns` prepared as shown in the release
workflow. Constructor builds only for the host platform; it does not
cross-compile native installers.
