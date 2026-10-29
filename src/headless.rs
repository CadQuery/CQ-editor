//! Terminal commands executed by the bundled Python worker without egui.

use std::process::Command;

use crate::{APP_ID, embedded, worker};

struct Invocation {
    python: Option<String>,
    arguments: Vec<String>,
}

/// Run a headless command if present; leave ordinary file/open arguments to
/// the GUI parser.
pub fn run(args: &[String]) -> Option<i32> {
    let Invocation { python, arguments } = match invocation(args) {
        Ok(Some(invocation)) => invocation,
        Ok(None) => return None,
        Err(message) => {
            eprintln!("{message}");
            return Some(2);
        }
    };

    let python = worker::launch::find_python(python.as_deref());
    let cache = eframe::storage_dir(APP_ID).unwrap_or_else(|| std::env::temp_dir().join(APP_ID));
    let worker_dir = match embedded::worker_dir(&cache) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("Cannot prepare Python worker: {error}");
            return Some(1);
        }
    };
    let mut python_paths = vec![worker_dir];
    if let Some(existing) = std::env::var_os("PYTHONPATH") {
        python_paths.extend(std::env::split_paths(&existing));
    }
    let python_path = match std::env::join_paths(python_paths) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("Cannot set PYTHONPATH: {error}");
            return Some(1);
        }
    };
    // Release builds use the Windows GUI subsystem. Attach the shell before
    // launching Python so its inherited stdout/stderr reach the terminal.
    #[cfg(all(windows, not(debug_assertions)))]
    unsafe {
        use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
    let status = Command::new(&python)
        .arg("-m")
        .arg("cq_worker.cli")
        .args(arguments)
        .env("PYTHONPATH", python_path)
        .env("PYTHONIOENCODING", "utf-8")
        .status();
    Some(match status {
        Ok(status) => status.code().unwrap_or(1),
        Err(error) => {
            eprintln!("Cannot start {}: {error}", python.display());
            1
        }
    })
}

fn invocation(args: &[String]) -> Result<Option<Invocation>, String> {
    let mut python = None;
    let mut forwarded = Vec::new();
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--python" {
            python = Some(args.get(index + 1).ok_or("--python needs a path")?.clone());
            index += 2;
        } else {
            forwarded.push(args[index].clone());
            index += 1;
        }
    }
    if matches!(
        forwarded.first().map(String::as_str),
        Some("build" | "check")
    ) {
        Ok(Some(Invocation {
            python,
            arguments: forwarded,
        }))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
#[path = "../tests/rust/app/test_headless.rs"]
mod tests;
