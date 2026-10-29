use std::time::{Duration, Instant};

use super::*;
use crate::worker::launch::find_python;

#[test]
fn package_lists_are_names_versions_or_wheels_never_options() {
    assert_eq!(
        parse_packages(" cadquery, jedi\n ocp-vscode>=2.0  trimesh[easy] ").unwrap(),
        ["cadquery", "jedi", "ocp-vscode>=2.0", "trimesh[easy]"]
    );
    assert_eq!(
        parse_packages(r"C:\wheels\demo-1.0-py3-none-any.whl").unwrap(),
        [r"C:\wheels\demo-1.0-py3-none-any.whl"]
    );
    assert!(parse_packages("  ").is_err());
    for refused in [
        "--index-url http://x",
        "cadquery -e .",
        "a;b",
        "a|b",
        "a&b",
        "$(rm)",
        "\"a\"",
    ] {
        assert!(parse_packages(refused).is_err(), "{refused}");
    }
}

#[test]
fn creating_an_environment_runs_venv_then_its_own_pip() {
    let packages = vec!["cadquery".to_owned(), "jedi".to_owned()];
    let folder = Path::new("envs").join("cq");
    let steps = create_steps(Path::new("base-python"), &folder, &packages);
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0].program, Path::new("base-python"));
    assert_eq!(steps[0].args, ["-m", "venv", folder.to_str().unwrap()]);
    let python = venv_python(&folder);
    assert!(python.starts_with(&folder));
    assert_eq!(steps[1].program, python);
    assert_eq!(steps[1].args, ["-m", "pip", "install", "cadquery", "jedi"]);
}

/// The Python CQ-editor itself would start, when there is one.
fn python() -> Option<PathBuf> {
    let python = find_python(None);
    let works = Command::new(&python)
        .args(["-c", "pass"])
        .status()
        .is_ok_and(|status| status.success());
    if !works {
        eprintln!("no Python: skipping the job test");
    }
    works.then_some(python)
}

fn run(step: &Path, code: &str) -> Step {
    Step {
        title: format!("python -c {code}"),
        program: step.to_path_buf(),
        args: vec!["-c".into(), code.into()],
    }
}

fn finish(job: &mut Job, limit: Duration) -> JobState {
    let started = Instant::now();
    while *job.poll() == JobState::Running {
        assert!(started.elapsed() < limit, "the job did not finish");
        thread::sleep(Duration::from_millis(20));
    }
    job.state().clone()
}

#[test]
fn a_job_runs_its_steps_in_order_and_stops_at_a_failure() {
    let Some(python) = python() else { return };
    let mut job = Job::start(vec![
        run(&python, "print('first')"),
        run(&python, "import sys; print('to stderr', file=sys.stderr)"),
        run(&python, "import sys; sys.exit(3)"),
        run(&python, "print('never')"),
    ]);
    let state = finish(&mut job, Duration::from_secs(60));
    let JobState::Failed(problem) = state else {
        panic!("{state:?}");
    };
    assert!(problem.contains("code 3"), "{problem}");
    let output = job.output().join("\n");
    assert!(output.contains("first") && output.contains("to stderr"));
    assert!(!output.contains("never"), "later steps are skipped");
    assert!(output.find("first") < output.find("to stderr"));

    let mut job = Job::start(vec![run(&python, "print('ok')")]);
    assert_eq!(
        finish(&mut job, Duration::from_secs(60)),
        JobState::Succeeded
    );
}

#[test]
fn a_cancelled_job_stops_its_command_and_skips_the_rest() {
    let Some(python) = python() else { return };
    let mut job = Job::start(vec![
        run(
            &python,
            "print('started', flush=True); import time; time.sleep(60)",
        ),
        run(&python, "print('never')"),
    ]);
    let started = Instant::now();
    while !job.output().iter().any(|line| line == "started") {
        job.poll();
        assert!(started.elapsed() < Duration::from_secs(30));
        thread::sleep(Duration::from_millis(20));
    }
    job.cancel();
    assert_eq!(
        finish(&mut job, Duration::from_secs(20)),
        JobState::Cancelled
    );
    assert!(!job.output().iter().any(|line| line == "never"));
}

#[test]
fn an_environment_is_created_and_gets_a_package_without_the_network() {
    let Some(base) = python() else { return };
    let can_create = Command::new(&base)
        .args(["-c", "import ensurepip, venv"])
        .status()
        .is_ok_and(|status| status.success());
    if !can_create {
        eprintln!("this Python cannot create environments (no ensurepip): skipping");
        return;
    }
    let dir = std::env::temp_dir().join(format!("cq-editor-env-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    // A minimal wheel, so pip installs it from the file alone.
    let wheel = dir.join("demo_package-1.0-py3-none-any.whl");
    let build = format!(
        "import zipfile\n\
         files = {{\n\
         'demo_package/__init__.py': 'VALUE = 42\\n',\n\
         'demo_package-1.0.dist-info/METADATA': 'Metadata-Version: 2.1\\nName: demo-package\\nVersion: 1.0\\n',\n\
         'demo_package-1.0.dist-info/WHEEL': 'Wheel-Version: 1.0\\nGenerator: test\\nRoot-Is-Purelib: true\\nTag: py3-none-any\\n',\n\
         'demo_package-1.0.dist-info/RECORD': '',\n\
         }}\n\
         with zipfile.ZipFile(r'{}', 'w') as archive:\n\
         \x20   for name, text in files.items():\n\
         \x20       archive.writestr(name, text)\n",
        wheel.display()
    );
    let built = Command::new(&base).args(["-c", &build]).status().unwrap();
    assert!(built.success());

    let folder = dir.join("env");
    let packages = vec![wheel.display().to_string()];
    let mut job = Job::start(create_steps(&base, &folder, &packages));
    let state = finish(&mut job, Duration::from_secs(300));
    assert_eq!(state, JobState::Succeeded, "{}", job.output().join("\n"));

    let output = Command::new(venv_python(&folder))
        .args(["-c", "import demo_package; print(demo_package.VALUE)"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "42");
    let _ = std::fs::remove_dir_all(&dir);
}
