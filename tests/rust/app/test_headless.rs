use super::*;

#[test]
fn routes_commands_and_interpreter_without_swallowing_gui_files() {
    let args = [
        "--python",
        "custom-python",
        "build",
        "part.py",
        "--export",
        "part.step",
    ]
    .map(str::to_owned);
    let command = invocation(&args).unwrap().unwrap();
    assert_eq!(command.python.as_deref(), Some("custom-python"));
    assert_eq!(
        command.arguments,
        ["build", "part.py", "--export", "part.step"]
    );
    assert!(invocation(&["part.py".into()]).unwrap().is_none());
}
