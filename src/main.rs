//! CQ-editor: a CadQuery script editor and 3D viewer.
//!
//! The interface is written in Rust (egui + wgpu); CadQuery itself runs in a
//! Python worker process (`worker/cq_worker`).

// No console window for release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod commands;
mod dialogs;
mod document;
mod editor;
mod embedded;
mod environments;
mod file_watcher;
mod headless;
mod logging;
mod panels;
mod parameters;
mod project;
mod recovery;
mod scene;
mod settings;
mod spatial;
mod theme;
mod ui;
mod updates;
mod viewer;
mod worker;

use std::path::PathBuf;

use app::{CqEditorApp, StartupOptions};

const USAGE: &str = "Usage: cq-editor [--python PATH] [FILE]
       cq-editor [--python PATH] build FILE --export OUTPUT [--set NAME=VALUE]... [--unit UNIT] [--json]
       cq-editor [--python PATH] check FILE [--set NAME=VALUE]... [--json]

  FILE            CadQuery script to open
  --python PATH   Python interpreter with CadQuery (overrides the preferences)";

/// Name of the data folder (settings, recovery, logs) and of the app.
pub const APP_ID: &str = "cq-editor";

fn main() -> eframe::Result {
    logging::init();

    let args: Vec<_> = std::env::args().skip(1).collect();
    if let Some(status) = headless::run(&args) {
        std::process::exit(status);
    }

    let options = match parse_args(args.into_iter()) {
        Ok(options) => options,
        Err(message) => {
            show_cli_message(&format!("{message}\n\n{USAGE}"), true);
            std::process::exit(2);
        }
    };

    if let Some(data) = eframe::storage_dir(APP_ID) {
        match logging::init_file(&data.join("logs")) {
            Ok(path) => log::debug!("Writing the log to {}", path.display()),
            Err(error) => log::warn!("Cannot write a log file: {error}"),
        }
    }

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("CQ-editor")
            .with_app_id("cq-editor")
            .with_inner_size([1280.0, 780.0])
            .with_min_inner_size([760.0, 480.0])
            .with_icon(window_icon()),
        persist_window: true,
        centered: true,
        ..Default::default()
    };

    let result = eframe::run_native(
        APP_ID,
        native_options,
        Box::new(move |cc| Ok(Box::new(CqEditorApp::new(cc, options)))),
    );
    logging::flush();
    result
}

/// Shows command-line output: usage and argument errors.
fn show_cli_message(message: &str, is_error: bool) {
    #[cfg(all(windows, not(debug_assertions)))]
    if windows_console::show(message, is_error) {
        return;
    }
    if is_error {
        eprintln!("{message}");
    } else {
        println!("{message}");
    }
}

/// Release builds on Windows are GUI programs without a console of their
/// own, so standard output goes nowhere unless a shell redirected it.
#[cfg(all(windows, not(debug_assertions)))]
mod windows_console {
    use std::io::Write as _;

    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Console::{
        ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE,
    };

    /// Writes `message` to the console of the shell that started the
    /// program, or shows it in a message box when there is none (for
    /// example when started from Explorer). Returns `false` when standard
    /// output is redirected, so plain printing works.
    pub fn show(message: &str, is_error: bool) -> bool {
        let stream = if is_error {
            STD_ERROR_HANDLE
        } else {
            STD_OUTPUT_HANDLE
        };
        // SAFETY: plain Win32 calls without pointer arguments.
        let handle = unsafe { GetStdHandle(stream) };
        if !handle.is_null() && handle != INVALID_HANDLE_VALUE {
            return false;
        }
        // SAFETY: as above.
        if unsafe { AttachConsole(ATTACH_PARENT_PROCESS) } != 0
            && let Ok(mut console) = std::fs::OpenOptions::new().write(true).open("CONOUT$")
        {
            // The shell has already printed its next prompt.
            let text = message.replace('\n', "\r\n");
            let _ = write!(console, "\r\n{text}\r\n");
            return true;
        }
        rfd::MessageDialog::new()
            .set_title("CQ-editor")
            .set_description(message)
            .set_level(if is_error {
                rfd::MessageLevel::Error
            } else {
                rfd::MessageLevel::Info
            })
            .show();
        true
    }
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<StartupOptions, String> {
    let mut options = StartupOptions::default();
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                show_cli_message(USAGE, false);
                std::process::exit(0);
            }
            "--python" => {
                options.python = Some(args.next().ok_or("--python needs a path")?);
            }
            flag if flag.starts_with('-') => return Err(format!("Unknown option {flag}")),
            file if options.file.is_none() => options.file = Some(PathBuf::from(file)),
            extra => return Err(format!("Unexpected argument {extra}")),
        }
    }
    Ok(options)
}

fn window_icon() -> egui::IconData {
    match image::load_from_memory_with_format(app::LOGO_ICO, image::ImageFormat::Ico) {
        Ok(image) => {
            let image = image
                .resize(128, 128, image::imageops::FilterType::Lanczos3)
                .to_rgba8();
            egui::IconData {
                width: image.width(),
                height: image.height(),
                rgba: image.into_raw(),
            }
        }
        Err(err) => {
            log::warn!("Cannot decode the application icon: {err}");
            egui::IconData::default()
        }
    }
}

#[cfg(test)]
#[path = "../tests/rust/app/test_cli.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/rust/app/test_compatibility.rs"]
mod compatibility_tests;
