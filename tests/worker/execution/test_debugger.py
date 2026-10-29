import textwrap

from cq_worker.debugger import Action, Debugger
from cq_worker.execution import SCRIPT_FILENAME, compile_script
from cq_worker.execution import Execution, RunSettings

SOURCE = textwrap.dedent("""\
    def helper(x):
        y = x * 2
        return y

    a = 1
    b = helper(a)
    c = b + 1
    """)


def trace(breakpoints, actions):
    """Run SOURCE, answering each pause with the next action; return pause lines."""

    lines = []
    script_actions = iter(actions)

    def on_pause(frame):
        lines.append(frame.f_lineno)
        return next(script_actions, Action.CONTINUE)

    namespace = {}
    Debugger(SCRIPT_FILENAME, breakpoints, on_pause).run(
        compile_script(SOURCE), namespace
    )
    return lines, namespace


def test_runs_to_breakpoints_only():
    lines, namespace = trace({6}, [])
    assert lines == [6]
    assert namespace["c"] == 3


def test_step_over_skips_function_bodies():
    lines, _ = trace({5}, [Action.STEP, Action.STEP, Action.CONTINUE])
    assert lines == [5, 6, 7]


def test_step_in_and_out():
    lines, _ = trace({6}, [Action.STEP_IN, Action.STEP_OUT, Action.CONTINUE])
    # Step in lands on the first line of helper, step out back in the caller.
    assert lines[:2] == [6, 2]
    assert lines[2] in (6, 7)


def test_stop_aborts_the_script():
    from cq_worker.debugger import DebugStop

    try:
        trace({5}, [Action.STOP])
    except DebugStop:
        pass
    else:
        raise AssertionError("DebugStop expected")


def test_stop_is_not_swallowed_by_the_scripts_exception_handlers():
    from cq_worker.debugger import DebugStop

    source = textwrap.dedent("""\
        reached = []
        for attempt in range(3):
            try:
                reached.append(attempt)
            except Exception:
                pass
        """)
    namespace = {}
    pauses = []

    def on_pause(frame):
        pauses.append(frame.f_lineno)
        return Action.STOP

    try:
        Debugger(SCRIPT_FILENAME, {4}, on_pause).run(compile_script(source), namespace)
    except DebugStop:
        pass
    else:
        raise AssertionError("DebugStop expected")
    assert pauses == [4]
    assert namespace["reached"] == []


def test_project_module_breakpoints_pause_and_can_be_updated(tmp_path):
    module = tmp_path / "helper_req08.py"
    module.write_text("def work(value):\n    doubled = value * 2\n    return doubled\n")
    execution = Execution(
        "import helper_req08\nanswer = helper_req08.work(4)\n",
        str(tmp_path / "main.py"),
        RunSettings(),
    )
    pauses = []

    def on_pause(frame):
        pauses.append((frame.f_code.co_filename, frame.f_lineno))
        debugger.set_breakpoints([], {})
        return Action.CONTINUE

    debugger = Debugger(
        SCRIPT_FILENAME, [], on_pause, str(tmp_path), {str(module): [2, 3]}
    )
    assert execution.run(debugger.run), execution.error
    assert pauses == [(str(module), 2)]
    assert execution.namespace["answer"] == 8
