from path import Path
import os, sys, asyncio

if sys.platform == 'win32':
    asyncio.set_event_loop_policy(asyncio.WindowsSelectorEventLoopPolicy())

from multiprocessing import Process

import pytest
import pytestqt
import cadquery as cq

from PyQt5.QtCore import Qt, QSettings
from PyQt5.QtWidgets import QFileDialog, QMessageBox

from cq_editor.__main__ import MainWindow
from cq_editor.widgets.editor import Editor
from cq_editor.cq_utils import export, get_occ_color

code = \
'''import cadquery as cq
result = cq.Workplane("XY" )
result = result.box(3, 3, 0.5)
result = result.edges("|Z").fillet(0.125)'''

code_bigger_object = \
'''import cadquery as cq
result = cq.Workplane("XY" )
result = result.box(20, 20, 0.5)
result = result.edges("|Z").fillet(0.125)
'''

code_show_Workplane = \
'''import cadquery as cq
result = cq.Workplane("XY" )
result = result.box(3, 3, 0.5)
result = result.edges("|Z").fillet(0.125)

show_object(result)
'''

code_show_Workplane_named = \
'''import cadquery as cq
result = cq.Workplane("XY" )
result = result.box(3, 3, 0.5)
result = result.edges("|Z").fillet(0.125)

log('test')
show_object(result,name='test')
'''

code_show_Shape = \
'''import cadquery as cq
result = cq.Workplane("XY" )
result = result.box(3, 3, 0.5)
result = result.edges("|Z").fillet(0.125)

show_object(result.val())
'''

code_debug_Workplane = \
'''import cadquery as cq
result = cq.Workplane("XY" )
result = result.box(3, 3, 0.5)
result = result.edges("|Z").fillet(0.125)

debug(result)
'''

code_multi = \
'''import cadquery as cq
result1 = cq.Workplane("XY" ).box(3, 3, 0.5)
result2 = cq.Workplane("XY" ).box(3, 3, 0.5).translate((0,15,0))
'''

code_nested_top = """import test_nested_bottom
"""

code_nested_bottom = """a=1
"""

code_reload_issue = """wire0 = cq.Workplane().lineTo(5, 5).lineTo(10, 0).close().val()
solid1 = cq.Solid.extrudeLinear(cq.Face.makeFromWires(wire0), cq.Vector(0, 0, 1))
r1 = cq.Workplane(solid1).translate((10, 0, 0))
"""

def _modify_file(code, path="test.py"):
    with open(path, "w", 1) as f:
        f.write(code)


def modify_file(code, path="test.py"):
    p = Process(target=_modify_file, args=(code,path))
    p.start()
    p.join()

def get_center(widget):

    pos = widget.pos()
    pos.setX(pos.x()+widget.width()//2)
    pos.setY(pos.y()+widget.height()//2)

    return pos

def get_bottom_left(widget):

    pos = widget.pos()
    pos.setY(pos.y()+widget.height())

    return pos

def get_rgba(ais):

    alpha = ais.Transparency()
    color = get_occ_color(ais)

    return color.redF(), color.greenF(), color.blueF(), alpha

@pytest.fixture
def main(qtbot,mocker):

    mocker.patch.object(QMessageBox, 'question', return_value=QMessageBox.Yes)

    win = MainWindow()
    win.show()

    qtbot.addWidget(win)

    editor = win.components['editor']
    editor.set_text(code)

    debugger = win.components['debugger']
    debugger._actions['Run'][0].triggered.emit()

    return qtbot, win

@pytest.fixture
def main_clean(qtbot,mocker):

    mocker.patch.object(QMessageBox, 'question', return_value=QMessageBox.Yes)

    win = MainWindow()
    win.show()

    qtbot.addWidget(win)
    qtbot.waitForWindowShown(win)

    editor = win.components['editor']
    editor.set_text(code)

    return qtbot, win

@pytest.fixture
def main_clean_do_not_close(qtbot,mocker):

    mocker.patch.object(QMessageBox, 'question', return_value=QMessageBox.No)

    win = MainWindow()
    win.show()

    qtbot.addWidget(win)
    qtbot.waitForWindowShown(win)

    editor = win.components['editor']
    editor.set_text(code)

    return qtbot, win

@pytest.fixture
def main_multi(qtbot,mocker):

    mocker.patch.object(QMessageBox, 'question', return_value=QMessageBox.Yes)
    mocker.patch.object(QFileDialog, 'getSaveFileName', return_value=('out.step',''))

    win = MainWindow()
    win.show()

    qtbot.addWidget(win)
    qtbot.waitForWindowShown(win)

    editor = win.components['editor']
    editor.set_text(code_multi)

    debugger = win.components['debugger']
    debugger._actions['Run'][0].triggered.emit()

    return qtbot, win

def test_render(main):

    qtbot, win = main

    obj_tree_comp = win.components['object_tree']
    editor = win.components['editor']
    debugger = win.components['debugger']
    console = win.components['console']
    log = win.components['log']

    # enable CQ reloading
    debugger.preferences['Reload CQ'] = True

    # check that object was rendered
    assert(obj_tree_comp.CQ.childCount() == 1)

    # check that object was removed
    obj_tree_comp._toolbar_actions[0].triggered.emit()
    assert(obj_tree_comp.CQ.childCount() == 0)

    # check that object was rendered usin explicit show_object call
    editor.set_text(code_show_Workplane)
    debugger._actions['Run'][0].triggered.emit()

    assert(obj_tree_comp.CQ.childCount() == 1)

    obj_tree_comp._toolbar_actions[0].triggered.emit()
    assert(obj_tree_comp.CQ.childCount() == 0)

    # check that cq.Shape object was rendered using explicit show_object call
    editor.set_text(code_show_Shape)
    debugger._actions['Run'][0].triggered.emit()

    assert(obj_tree_comp.CQ.childCount() == 1)

    obj_tree_comp._toolbar_actions[0].triggered.emit()
    assert(obj_tree_comp.CQ.childCount() == 0)

    # test rendering via console
    console.execute(code_show_Workplane)
    assert(obj_tree_comp.CQ.childCount() == 1)

    obj_tree_comp._toolbar_actions[0].triggered.emit()
    assert(obj_tree_comp.CQ.childCount() == 0)

    console.execute(code_show_Shape)
    assert(obj_tree_comp.CQ.childCount() == 1)

    # check object rendering using show_object call with a name specified and
    # debug call
    editor.set_text(code_show_Workplane_named)
    debugger._actions['Run'][0].triggered.emit()

    qtbot.wait(100)
    assert(obj_tree_comp.CQ.child(0).text(0) == 'test')
    assert('test' in log.toPlainText().splitlines()[-1])

    # cq reloading check
    obj_tree_comp._toolbar_actions[0].triggered.emit()
    assert(obj_tree_comp.CQ.childCount() == 0)

    editor.set_text(code_reload_issue)
    debugger._actions['Run'][0].triggered.emit()

    qtbot.wait(100)
    assert(obj_tree_comp.CQ.childCount() == 1)

    debugger._actions['Run'][0].triggered.emit()
    qtbot.wait(100)
    assert(obj_tree_comp.CQ.childCount() == 1)

def test_export(main,mocker):

    qtbot, win = main

    debugger = win.components['debugger']
    debugger._actions['Run'][0].triggered.emit()

    #set focus
    obj_tree = win.components['object_tree'].tree
    obj_tree_comp = win.components['object_tree']
    qtbot.mouseClick(obj_tree, Qt.LeftButton)
    qtbot.keyClick(obj_tree, Qt.Key_Down)
    qtbot.keyClick(obj_tree, Qt.Key_Down)

    #export STL
    mocker.patch.object(QFileDialog, 'getSaveFileName', return_value=('out.stl',''))
    obj_tree_comp._export_STL_action.triggered.emit()
    assert(os.path.isfile('out.stl'))

    #export STEP
    mocker.patch.object(QFileDialog, 'getSaveFileName', return_value=('out.step',''))
    obj_tree_comp._export_STEP_action.triggered.emit()
    assert(os.path.isfile('out.step'))

    #clean
    os.remove('out.step')
    os.remove('out.stl')

def number_visible_items(viewer):

    from OCP.AIS import AIS_ListOfInteractive
    l = AIS_ListOfInteractive()

    viewer_ctx = viewer._get_context()
    viewer_ctx.DisplayedObjects(l)

    return l.Extent()

def test_inspect(main):

    qtbot, win = main

    #set focus and make invisible
    obj_tree = win.components['object_tree'].tree
    qtbot.mouseClick(obj_tree, Qt.LeftButton)
    qtbot.keyClick(obj_tree, Qt.Key_Down)
    qtbot.keyClick(obj_tree, Qt.Key_Down)
    qtbot.keyClick(obj_tree, Qt.Key_Space)

    #enable object inspector
    insp = win.components['cq_object_inspector']
    insp._toolbar_actions[0].toggled.emit(True)

    #check if all stack items are visible in the tree
    assert(insp.root.childCount() == 3)

    #check if correct number of items is displayed
    viewer = win.components['viewer']

    insp.setCurrentItem(insp.root.child(0))
    assert(number_visible_items(viewer) == 4)

    insp.setCurrentItem(insp.root.child(1))
    assert(number_visible_items(viewer) == 7)

    insp.setCurrentItem(insp.root.child(2))
    assert(number_visible_items(viewer) == 4)

    insp._toolbar_actions[0].toggled.emit(False)
    assert(number_visible_items(viewer) == 3)

class event_loop(object):
    '''Used to mock the QEventLoop for the debugger component
    '''

    def __init__(self,callbacks):

        self.callbacks = callbacks
        self.i = 0

    def exec_(self):

        if self.i<len(self.callbacks):
            self.callbacks[self.i]()
            self.i+=1

    def exit(self,*args):

        pass

def patch_debugger(debugger,event_loop_mock):

        debugger.inner_event_loop.exec_ = event_loop_mock.exec_
        debugger.inner_event_loop.exit = event_loop_mock.exit

def test_debug(main,mocker):

    # store the tracing function
    trace_function = sys.gettrace()

    def assert_func(x):
        '''Neddedd to perform asserts in lambdas
        '''
        assert(x)

    qtbot, win = main

    #clear all
    obj_tree = win.components['object_tree']
    obj_tree.toolbarActions()[0].triggered.emit()

    editor = win.components['editor']
    editor.set_text(code)

    debugger = win.components['debugger']
    actions = debugger._actions['Run']
    run,debug,step,step_in,cont = actions

    variables = win.components['variables_viewer']

    traceback_view = win.components['traceback_viewer']

    def check_no_error_occured():
        '''check that no error occured while stepping through the debugger
        '''
        assert( '' == traceback_view.current_exception.text())

    viewer = win.components['viewer']
    assert(number_visible_items(viewer) == 3)

    #check breakpoints
    assert(debugger.breakpoints == [])

    #check _frames
    assert(debugger._frames == [])

    #test step through
    ev = event_loop([
        lambda: (
            assert_func(variables.model().rowCount() == 4),
            assert_func(number_visible_items(viewer) == 3),
            step.triggered.emit()),
        lambda: (
            assert_func(variables.model().rowCount() == 4),
            assert_func(number_visible_items(viewer) == 3),
            step.triggered.emit()),
        lambda: (
            assert_func(variables.model().rowCount() == 5),
            assert_func(number_visible_items(viewer) == 3),
            step.triggered.emit()),
        lambda: (
            assert_func(variables.model().rowCount() == 5),
            assert_func(number_visible_items(viewer) == 4),
            cont.triggered.emit())
        ])

    patch_debugger(debugger,ev)

    debug.triggered.emit(True)

    check_no_error_occured()

    assert(variables.model().rowCount() == 2)
    assert(number_visible_items(viewer) == 4)

    #test exit debug
    ev = event_loop([lambda: (step.triggered.emit(),),
                     lambda: (assert_func(variables.model().rowCount() == 4),
                              assert_func(number_visible_items(viewer) == 3),
                              debug.triggered.emit(False),)])

    patch_debugger(debugger,ev)

    debug.triggered.emit(True)

    check_no_error_occured()

    assert(variables.model().rowCount() == 1)
    assert(number_visible_items(viewer) == 3)

    #test breakpoint
    ev = event_loop([lambda: (cont.triggered.emit(),),
                     lambda: (assert_func(variables.model().rowCount() == 5),
                              assert_func(number_visible_items(viewer) == 4),
                              cont.triggered.emit(),)])

    patch_debugger(debugger,ev)

    editor.debugger.set_breakpoints([(4,None)])

    debug.triggered.emit(True)

    check_no_error_occured()

    assert(variables.model().rowCount() == 2)
    assert(number_visible_items(viewer) == 4)

    #test breakpoint without using singals
    ev = event_loop([lambda: (cont.triggered.emit(),),
                     lambda: (assert_func(variables.model().rowCount() == 5),
                              assert_func(number_visible_items(viewer) == 4),
                              cont.triggered.emit(),)])

    patch_debugger(debugger,ev)

    editor.debugger.set_breakpoints([(4,None)])

    debugger.debug(True)

    check_no_error_occured()

    assert(variables.model().rowCount() == 2)
    assert(number_visible_items(viewer) == 4)

    #test debug() without using singals
    ev = event_loop([lambda: (cont.triggered.emit(),),
                     lambda: (assert_func(variables.model().rowCount() == 5),
                              assert_func(number_visible_items(viewer) == 4),
                              cont.triggered.emit(),)])

    patch_debugger(debugger,ev)

    editor.set_text(code_debug_Workplane)
    editor.debugger.set_breakpoints([(4,None)])

    debugger.debug(True)

    check_no_error_occured()

    CQ = obj_tree.CQ

    # object 1 (defualt color)
    r,g,b,a = get_rgba(CQ.child(0).ais)
    assert( a == pytest.approx(0.2) )
    assert( r == 1.0 )

    assert(variables.model().rowCount() == 2)
    assert(number_visible_items(viewer) == 4)

    # restore the tracing function
    sys.settrace(trace_function)

code_err1 = \
'''import cadquery as cq
(
result = cq.Workplane("XY" ).box(3, 3, 0.5).edges("|Z").fillet(0.125)
'''

code_err2 = \
'''import cadquery as cq
result = cq.Workplane("XY" ).box(3, 3, 0.5).edges("|Z").fillet(0.125)
f()
'''

def test_traceback(main):

    # store the tracing function
    trace_function = sys.gettrace()

    qtbot, win = main

    editor = win.components['editor']
    debugger = win.components['debugger']
    traceback_view = win.components['traceback_viewer']

    actions = debugger._actions['Run']
    run,debug,step,step_in,cont = actions

    editor.set_text(code_err1)
    run.triggered.emit()

    assert('SyntaxError' in traceback_view.current_exception.text())

    debug.triggered.emit()

    assert('SyntaxError' in traceback_view.current_exception.text())
    assert(debug.isChecked() == False)

    editor.set_text(code_err2)
    run.triggered.emit()

    assert('NameError' in traceback_view.current_exception.text())
    assert(hasattr(sys, 'last_traceback'))

    del sys.last_traceback
    assert(not hasattr(sys, 'last_traceback'))


    #test last_traceback with debug
    ev = event_loop([lambda: (cont.triggered.emit(),)])
    patch_debugger(debugger,ev)

    debugger.debug(True)

    assert('NameError' in traceback_view.current_exception.text())
    assert(hasattr(sys, 'last_traceback'))

    # restore the tracing function
    sys.settrace(trace_function)

@pytest.fixture
def editor(qtbot):

    win = Editor()
    win.show()

    qtbot.addWidget(win)

    return qtbot, win

def conv_line_ends(text):

    return '\n'.join(text.splitlines())

def test_editor(monkeypatch,editor):

    qtbot, editor = editor

    with open('test.py','w') as f:
        f.write(code)

    #check that no text is present
    assert(editor.get_text_with_eol() == '')

    #check that loading from file works properly
    editor.load_from_file('test.py')
    assert(len(editor.get_text_with_eol()) > 0)
    assert(conv_line_ends(editor.get_text_with_eol()) == code)

    #check that loading from file works properly
    editor.new()
    assert(editor.get_text_with_eol() == '')

    #monkeypatch QFileDialog methods
    def filename(*args, **kwargs):
        return 'test.py',None

    def filename2(*args, **kwargs):
        return 'test2.py',None

    monkeypatch.setattr(QFileDialog, 'getOpenFileName',
                        staticmethod(filename))

    monkeypatch.setattr(QFileDialog, 'getSaveFileName',
                        staticmethod(filename2))

    #check that open file works properly
    editor.open()
    assert(conv_line_ends(editor.get_text_with_eol()) == code)

    #check that save file works properly
    editor.new()
    qtbot.mouseClick(editor, Qt.LeftButton)
    qtbot.keyClick(editor,Qt.Key_A)

    assert(editor.document().isModified() == True)

    editor.filename = 'test2.py'
    editor.save()

    assert(editor.document().isModified() == False)

    monkeypatch.setattr(QFileDialog, 'getOpenFileName',
                        staticmethod(filename2))

    editor.open()
    assert(editor.get_text_with_eol() == 'a')

    #check that save as works properly
    os.remove('test2.py')
    editor.save_as()
    assert(os.path.exists(filename2()[0]))

    #test persistance
    settings = QSettings('test')
    editor.saveComponentState(settings)

    editor.new()
    assert(editor.get_text_with_eol() == '')

    editor.restoreComponentState(settings)
    assert(editor.get_text_with_eol() == 'a')

    #test error handling
    os.remove('test2.py')
    assert(not os.path.exists('test2.py'))
    editor.restoreComponentState(settings)

@pytest.mark.repeat(1)
def test_editor_autoreload(monkeypatch,editor):

    qtbot, editor = editor

    TIMEOUT = 500

    # start out with autoreload enabled
    editor.autoreload(True)

    with open('test.py','w') as f:
        f.write(code)

    assert(editor.get_text_with_eol() == '')

    editor.load_from_file('test.py')
    assert(len(editor.get_text_with_eol()) > 0)

    # wait for reload.
    with qtbot.waitSignal(editor.triggerRerender, timeout=TIMEOUT):
        # modify file - NB: separate process is needed to avoid Widows quirks
        modify_file(code_bigger_object)

    # check that editor has updated file contents
    assert(code_bigger_object.splitlines()[2] in editor.get_text_with_eol())

    # disable autoreload
    editor.autoreload(False)

    # Wait for reload in case it incorrectly happens. A timeout should occur
    # instead because a re-render should not be triggered with autoreload
    # disabled.
    with pytest.raises(pytestqt.exceptions.TimeoutError):
        with qtbot.waitSignal(editor.triggerRerender, timeout=TIMEOUT):
            # re-write original file contents
            modify_file(code)

    # editor should continue showing old contents since autoreload is disabled.
    assert(code_bigger_object.splitlines()[2] in editor.get_text_with_eol())

    # Saving a file with autoreload disabled should not trigger a rerender.
    with pytest.raises(pytestqt.exceptions.TimeoutError):
        with qtbot.waitSignal(editor.triggerRerender, timeout=TIMEOUT):
            editor.save()

    editor.autoreload(True)

    # Saving a file with autoreload enabled should trigger a rerender.
    with qtbot.waitSignal(editor.triggerRerender, timeout=TIMEOUT):
        editor.save()

def test_autoreload_nested(editor):

    qtbot, editor = editor

    TIMEOUT = 500

    editor.autoreload(True)
    editor.preferences['Autoreload: watch imported modules'] = True

    with open('test_nested_top.py','w') as f:
        f.write(code_nested_top)

    with open('test_nested_bottom.py','w') as f:
        f.write("")

    assert(editor.get_text_with_eol() == '')

    editor.load_from_file('test_nested_top.py')
    assert(len(editor.get_text_with_eol()) > 0)

    # wait for reload.
    with qtbot.waitSignal(editor.triggerRerender, timeout=TIMEOUT):
        # modify file - NB: separate process is needed to avoid Windows quirks
        modify_file(code_nested_bottom, 'test_nested_bottom.py')

def test_console(main):

    qtbot, win = main

    console = win.components['console']

    # test execute_command
    a = []
    console.push_vars({'a' : a})
    console.execute_command('a.append(1)')
    assert(len(a) == 1)

    # test print_text
    pos_orig = console._prompt_pos
    console.print_text('a')
    assert(console._prompt_pos == pos_orig + len('a'))

def test_viewer(main):

    qtbot, win = main

    viewer = win.components['viewer']

    #not sure how to test this, so only smoke tests

    #trigger all 'View' actions
    actions = viewer._actions['View']
    for a in actions: a.trigger()

code_module = \
'''def dummy(): return True'''

code_import = \
'''from module import dummy
assert(dummy())'''

def test_module_import(main):

    qtbot, win = main

    editor = win.components['editor']
    debugger = win.components['debugger']
    traceback_view = win.components['traceback_viewer']

    #save the dummy module
    with open('module.py','w') as f:
        f.write(code_module)

    #run the code importing this module
    editor.set_text(code_import)
    debugger._actions['Run'][0].triggered.emit()

    #verify that no exception was generated
    assert(traceback_view.current_exception.text()  == '')

def test_auto_fit_view(main_clean):

    def concat(eye,proj,scale):
        return eye+proj+(scale,)

    def approx_view_properties(eye,proj,scale):

        return pytest.approx(eye+proj+(scale,))

    qtbot, win = main_clean

    editor = win.components['editor']
    debugger = win.components['debugger']
    viewer = win.components['viewer']
    object_tree = win.components['object_tree']

    view = viewer.canvas.view
    viewer.preferences['Fit automatically'] = False
    eye0,proj0,scale0 = view.Eye(),view.Proj(),view.Scale()
    # check if camera position is adjusted automatically when rendering for the
    # first time
    debugger.render()
    eye1,proj1,scale1 = view.Eye(),view.Proj(),view.Scale()
    assert( concat(eye0,proj0,scale0) != \
            approx_view_properties(eye1,proj1,scale1) )

    # check if camera position is not changed fter code change
    editor.set_text(code_bigger_object)
    debugger.render()
    eye2,proj2,scale2 = view.Eye(),view.Proj(),view.Scale()
    assert( concat(eye1,proj1,scale1) == \
            approx_view_properties(eye2,proj2,scale2) )

    # check if position is adjusted automatically after erasing all objects
    object_tree.removeObjects()
    debugger.render()
    eye3,proj3,scale3 = view.Eye(),view.Proj(),view.Scale()
    assert( concat(eye2,proj2,scale2) != \
            approx_view_properties(eye3,proj3,scale3) )

    # check if position is adjusted automatically if settings are changed
    viewer.preferences['Fit automatically'] = True
    editor.set_text(code)
    debugger.render()
    eye4,proj4,scale4 = view.Eye(),view.Proj(),view.Scale()
    assert( concat(eye3,proj3,scale3) != \
            approx_view_properties(eye4,proj4,scale4) )

def test_preserve_properties(main):
    qtbot, win = main

    debugger = win.components['debugger']
    debugger._actions['Run'][0].triggered.emit()

    object_tree = win.components['object_tree']
    object_tree.preferences['Preserve properties on reload'] = True

    assert(object_tree.CQ.childCount() == 1)
    props = object_tree.CQ.child(0).properties
    props['Visible'] = False
    props['Color'] = '#caffee'
    props['Alpha'] = 0.5

    debugger._actions['Run'][0].triggered.emit()

    assert(object_tree.CQ.childCount() == 1)
    props = object_tree.CQ.child(0).properties
    assert(props['Visible'] == False)
    assert(props['Color'].name() == '#caffee')
    assert(props['Alpha'] == 0.5)

def test_selection(main_multi,mocker):

    qtbot, win = main_multi

    viewer = win.components['viewer']
    object_tree = win.components['object_tree']

    CQ = object_tree.CQ
    obj1 = CQ.child(0)
    obj2 = CQ.child(1)

    # export with two selected objects
    obj1.setSelected(True)
    obj2.setSelected(True)

    object_tree._export_STEP_action.triggered.emit()
    imported = cq.importers.importStep('out.step')
    assert(len(imported.solids().vals()) == 2)

    # export with one selected objects
    obj2.setSelected(False)

    object_tree._export_STEP_action.triggered.emit()
    imported = cq.importers.importStep('out.step')
    assert(len(imported.solids().vals()) == 1)

    # export with one selected objects
    obj1.setSelected(False)
    CQ.setSelected(True)

    object_tree._export_STEP_action.triggered.emit()
    imported = cq.importers.importStep('out.step')
    assert(len(imported.solids().vals()) == 2)

    # check if viewer and object tree are properly connected
    CQ.setSelected(False)
    obj1.setSelected(True)
    obj2.setSelected(True)
    ctx = viewer._get_context()
    ctx.InitSelected()
    shapes = []

    while ctx.MoreSelected():
        shapes.append(ctx.SelectedShape())
        ctx.NextSelected()
    assert(len(shapes) == 2)

    viewer.fit()
    qtbot.mouseClick(viewer.canvas, Qt.LeftButton)

    assert(len(object_tree.tree.selectedItems()) == 0)

    viewer.sigObjectSelected.emit([obj1.shape_display.wrapped])
    assert(len(object_tree.tree.selectedItems()) == 1)

    # go through different handleSelection paths
    qtbot.mouseClick(object_tree.tree, Qt.LeftButton)
    qtbot.keyClick(object_tree.tree, Qt.Key_Down)
    qtbot.keyClick(object_tree.tree, Qt.Key_Down)
    qtbot.keyClick(object_tree.tree, Qt.Key_Down)
    qtbot.keyClick(object_tree.tree, Qt.Key_Down)

    assert(object_tree._export_STL_action.isEnabled() == False)
    assert(object_tree._export_STEP_action.isEnabled() == False)
    assert(object_tree._clear_current_action.isEnabled() == False)
    assert(object_tree.properties_editor.isEnabled() == False)

def test_closing(main_clean_do_not_close):

    qtbot,win = main_clean_do_not_close

    editor = win.components['editor']

    # make sure that windows is visible
    assert(win.isVisible())

    # should not quit
    win.close()
    assert(win.isVisible())

    # should quit
    editor.reset_modified()
    win.close()
    assert(not win.isVisible())

def test_check_for_updates(main,mocker):

    qtbot,win = main

    # patch requests
    import requests
    mocker.patch.object(requests.models.Response,'json',
                        return_value=[{'tag_name' : '0.0.2','draft' : False}])

    # stub QMessageBox about
    about_stub = mocker.stub()
    mocker.patch.object(QMessageBox, 'about', about_stub)

    import cadquery

    cadquery.__version__ = '0.0.1'
    win.check_for_cq_updates()
    assert(about_stub.call_args[0][1] == 'Updates available')

    cadquery.__version__ = '0.0.3'
    win.check_for_cq_updates()
    assert(about_stub.call_args[0][1] == 'No updates available')

@pytest.mark.skipif(sys.platform.startswith('linux'),reason='Segfault workaround for linux')
def test_screenshot(main,mocker):

    qtbot,win = main

    mocker.patch.object(QFileDialog, 'getSaveFileName', return_value=('out.png',''))

    viewer = win.components['viewer']
    viewer._actions['Tools'][0].triggered.emit()

    assert(os.path.exists('out.png'))

def test_resize(main):

    qtbot,win = main
    editor = win.components['editor']

    editor.hide()
    qtbot.wait(50)
    editor.show()
    qtbot.wait(50)

code_simple_step = \
'''import cadquery as cq
imported = cq.importers.importStep('shape.step')
'''

def test_relative_references(main):

    # create code with a relative reference in a subdirectory
    p = Path('test_relative_references')
    p.mkdir_p()
    p_code = p.joinpath('code.py')
    p_code.write_text(code_simple_step)
    # create the referenced step file
    shape = cq.Workplane("XY").box(1, 1, 1)
    p_step = p.joinpath('shape.step')
    export(shape, "step", p_step)
    # open code
    qtbot, win = main
    editor = win.components['editor']
    editor.load_from_file(p_code)
    # render
    debugger = win.components['debugger']
    debugger._actions['Run'][0].triggered.emit()
    # assert no errors
    traceback_view = win.components['traceback_viewer']
    assert(traceback_view.current_exception.text() == '')
    # assert one object has been rendered
    obj_tree_comp = win.components['object_tree']
    assert(obj_tree_comp.CQ.childCount() == 1)
    # clean up
    p_code.remove_p()
    p_step.remove_p()
    p.rmdir_p()


code_color = \
'''
import cadquery as cq
result = cq.Workplane("XY" ).box(1, 1, 1)

show_object(result, name ='1')
show_object(result, name ='2', options=dict(alpha=0.5,color='red'))
show_object(result, name ='3', options=dict(alpha=0.5,color='#ff0000'))
show_object(result, name ='4', options=dict(alpha=0.5,color=(255,0,0)))
show_object(result, name ='5', options=dict(alpha=0.5,color=(1.,0,0)))
show_object(result, name ='6', options=dict(rgba=(1.,0,0,.5)))
show_object(result, name ='7', options=dict(color=('ff','cc','dd')))
'''

def test_render_colors(main_clean):

    qtbot, win = main_clean

    obj_tree = win.components['object_tree']
    editor = win.components['editor']
    debugger = win.components['debugger']
    log = win.components['log']

    editor.set_text(code_color)
    debugger._actions['Run'][0].triggered.emit()

    CQ = obj_tree.CQ

    # object 1 (defualt color)
    assert not CQ.child(0).ais.HasColor()

    # object 2
    r,g,b,a = get_rgba(CQ.child(1).ais)
    assert( a == 0.5 )
    assert( r == 1.0 )
    assert( g == 0.0 )

    # object 3
    r,g,b,a = get_rgba(CQ.child(2).ais)
    assert( a == 0.5)
    assert( r == 1.0 )

    # object 4
    r,g,b,a = get_rgba(CQ.child(3).ais)
    assert( a == 0.5 )
    assert( r == 1.0 )

    # object 5
    r,g,b,a = get_rgba(CQ.child(4).ais)
    assert( a == 0.5 )
    assert( r == 1.0 )

    # object 6
    r,g,b,a = get_rgba(CQ.child(5).ais)
    assert( a == 0.5 )
    assert( r == 1.0 )

    # check if error occured
    qtbot.wait(100)
    assert('Unknown color format' in log.toPlainText().splitlines()[-1])

def test_render_colors_console(main_clean):

    qtbot, win = main_clean

    obj_tree = win.components['object_tree']
    log = win.components['log']
    console = win.components['console']

    console.execute_command(code_color)

    CQ = obj_tree.CQ

    # object 1 (defualt color)
    assert not CQ.child(0).ais.HasColor()

    # object 2
    r,g,b,a = get_rgba(CQ.child(1).ais)
    assert( a == 0.5 )
    assert( r == 1.0 )

    # object 3
    r,g,b,a = get_rgba(CQ.child(2).ais)
    assert( a == 0.5)
    assert( r == 1.0 )

    # object 4
    r,g,b,a = get_rgba(CQ.child(3).ais)
    assert( a == 0.5 )
    assert( r == 1.0 )

    # object 5
    r,g,b,a = get_rgba(CQ.child(4).ais)
    assert( a == 0.5 )
    assert( r == 1.0 )

    # object 6
    r,g,b,a = get_rgba(CQ.child(5).ais)
    assert( a == 0.5 )
    assert( r == 1.0 )

    # check if error occured
    qtbot.wait(100)
    assert('Unknown color format' in log.toPlainText().splitlines()[-1])

code_shading = \
'''
import cadquery as cq

res1 = cq.Workplane('XY').box(5, 7, 5)
res2 = cq.Workplane('XY').box(8, 5, 4)
show_object(res1)
show_object(res2,options={"alpha":0})
'''

def test_shading_aspect(main_clean):

    qtbot, win = main_clean

    obj_tree = win.components['object_tree']
    editor = win.components['editor']
    debugger = win.components['debugger']

    editor.set_text(code_shading)
    debugger._actions['Run'][0].triggered.emit()

    CQ = obj_tree.CQ

    # get material aspects
    ma1 = CQ.child(0).ais.Attributes().ShadingAspect().Material()
    ma2 = CQ.child(1).ais.Attributes().ShadingAspect().Material()

    # verify that they are the same
    assert ma1.Shininess() == ma2.Shininess()

def test_confirm_new(monkeypatch,editor):

    qtbot, editor = editor

    #check that initial state is as expected
    assert(editor.modified == False)

    editor.document().setPlainText(code)
    assert(editor.modified == True)

    #monkeypatch the confirmation dialog and run both scenarios
    def cancel(*args, **kwargs):
        return QMessageBox.No

    def ok(*args, **kwargs):
        return QMessageBox.Yes

    monkeypatch.setattr(QMessageBox, 'question',
                        staticmethod(cancel))

    editor.new()
    assert(editor.modified == True)
    assert(conv_line_ends(editor.get_text_with_eol()) == code)

    monkeypatch.setattr(QMessageBox, 'question',
                        staticmethod(ok))

    editor.new()
    assert(editor.modified == False)
    assert(editor.get_text_with_eol() == '')

code_show_topods = \
'''
import cadquery as cq
result = cq.Workplane("XY" ).box(1, 1, 1)

show_object(result.val().wrapped)
'''

def test_render_topods(main):

    qtbot, win = main

    obj_tree_comp = win.components['object_tree']
    editor = win.components['editor']
    debugger = win.components['debugger']
    console = win.components['console']

    # check that object was rendered
    assert(obj_tree_comp.CQ.childCount() == 1)

    # check that object was removed
    obj_tree_comp._toolbar_actions[0].triggered.emit()
    assert(obj_tree_comp.CQ.childCount() == 0)

    # check that object was rendered usin explicit show_object call
    editor.set_text(code_show_topods)
    debugger._actions['Run'][0].triggered.emit()
    assert(obj_tree_comp.CQ.childCount() == 1)

    # test rendering of topods object via console
    console.execute('show(result.val().wrapped)')
    assert(obj_tree_comp.CQ.childCount() == 2)

    # test rendering of list of topods object via console
    console.execute('show([result.val().wrapped,result.val().wrapped])')
    assert(obj_tree_comp.CQ.childCount() == 3)


code_show_shape_list = \
'''
import cadquery as cq
result1 = cq.Workplane("XY" ).box(1, 1, 1).val()
result2 = cq.Workplane("XY",origin=(0,1,1)).box(1, 1, 1).val()

show_object(result1)
show_object([result1,result2])
'''

def test_render_shape_list(main):

    qtbot, win = main

    log = win.components['log']

    obj_tree_comp = win.components['object_tree']
    editor = win.components['editor']
    debugger = win.components['debugger']
    console = win.components['console']

    # check that object was removed
    obj_tree_comp._toolbar_actions[0].triggered.emit()
    assert(obj_tree_comp.CQ.childCount() == 0)

    # check that object was rendered usin explicit show_object call
    editor.set_text(code_show_shape_list)
    debugger._actions['Run'][0].triggered.emit()
    assert(obj_tree_comp.CQ.childCount() == 2)

    # test rendering of Shape via console
    console.execute('show(result1)')
    console.execute('show([result1,result2])')
    assert(obj_tree_comp.CQ.childCount() == 4)

    # smoke test exception in show
    console.execute('show("a")')

code_show_assy = \
'''import cadquery as cq
result1 = cq.Workplane("XY" ).box(3, 3, 0.5)
assy = cq.Assembly(result1)

show_object(assy)
'''

def test_render_assy(main):

    qtbot, win = main

    obj_tree_comp = win.components['object_tree']
    editor = win.components['editor']
    debugger = win.components['debugger']
    console = win.components['console']

    # check that object was removed
    obj_tree_comp._toolbar_actions[0].triggered.emit()
    assert(obj_tree_comp.CQ.childCount() == 0)

    # check that object was rendered usin explicit show_object call
    editor.set_text(code_show_assy)
    debugger._actions['Run'][0].triggered.emit()
    qtbot.wait(500)
    assert(obj_tree_comp.CQ.childCount() == 1)

    # test rendering via console
    console.execute('show(assy)')
    qtbot.wait(500)
    assert(obj_tree_comp.CQ.childCount() == 2)

code_show_ais = \
'''import cadquery as cq
from cadquery.occ_impl.assembly import toCAF

import OCP

result1 = cq.Workplane("XY" ).box(3, 3, 0.5)
assy = cq.Assembly(result1)

lab, doc = toCAF(assy)
ais = OCP.XCAFPrs.XCAFPrs_AISObject(lab)

show_object(ais)
'''

def test_render_ais(main):

    qtbot, win = main

    obj_tree_comp = win.components['object_tree']
    editor = win.components['editor']
    debugger = win.components['debugger']
    console = win.components['console']

    # check that object was removed
    obj_tree_comp._toolbar_actions[0].triggered.emit()
    assert(obj_tree_comp.CQ.childCount() == 0)

    # check that object was rendered usin explicit show_object call
    editor.set_text(code_show_ais)
    debugger._actions['Run'][0].triggered.emit()
    qtbot.wait(500)
    assert(obj_tree_comp.CQ.childCount() == 1)

    # test rendering via console
    console.execute('show(ais)')
    qtbot.wait(500)
    assert(obj_tree_comp.CQ.childCount() == 2)

code_show_sketch = \
'''import cadquery as cq

s1 = cq.Sketch().rect(1,1)
s2 = cq.Sketch().segment((0,0), (0,3.),"s1")

show_object(s1)
show_object(s2)
'''

def test_render_sketch(main):

    qtbot, win = main

    obj_tree_comp = win.components['object_tree']
    editor = win.components['editor']
    debugger = win.components['debugger']
    console = win.components['console']

    # check that object was removed
    obj_tree_comp._toolbar_actions[0].triggered.emit()
    assert(obj_tree_comp.CQ.childCount() == 0)

    # check that object was rendered usin explicit show_object call
    editor.set_text(code_show_sketch)
    debugger._actions['Run'][0].triggered.emit()
    qtbot.wait(500)
    assert(obj_tree_comp.CQ.childCount() == 2)

    # test rendering via console
    console.execute('show(s1); show(s2)')
    qtbot.wait(500)
    assert(obj_tree_comp.CQ.childCount() == 4)

def test_window_title(monkeypatch, main):

    fname = 'test_window_title.py'

    with open(fname, 'w') as f:
        f.write(code)

    qtbot, win = main

    #monkeypatch QFileDialog methods
    def filename(*args, **kwargs):
        return fname, None

    monkeypatch.setattr(QFileDialog, 'getOpenFileName',
                        staticmethod(filename))

    win.components["editor"].open()
    assert(win.windowTitle().endswith(fname))

    # handle a new file
    win.components["editor"].new()
    # I don't really care what the title is, as long as it's not a filename
    assert(not win.windowTitle().endswith('.py'))

def test_module_discovery(tmp_path, editor):

    qtbot, editor = editor
    with open(tmp_path.joinpath('main.py'), 'w') as f:
        f.write('import b')

    assert editor.get_imported_module_paths(str(tmp_path.joinpath('main.py'))) == []

    tmp_path.joinpath('b.py').touch()

    assert editor.get_imported_module_paths(str(tmp_path.joinpath('main.py'))) == [str(tmp_path.joinpath('b.py'))]

def test_launch_syntax_error(tmp_path):

    # verify app launches when input file is bad
    win = MainWindow()

    inputfile = Path(tmp_path).joinpath("syntax_error.py")
    modify_file("print(", inputfile)
    editor = win.components["editor"]
    editor.autoreload(True)
    editor.preferences["Autoreload: watch imported modules"] = True
    editor.load_from_file(inputfile)

    win.show()
    assert(win.isVisible())

code_import_module_makebox = \
"""
from module_makebox import *
z = 1
r = makebox(z)
"""

code_module_makebox = \
"""
import cadquery as cq
def makebox(z):
    zval = z + 1
    return cq.Workplane().box(1, 1, zval)
"""

def test_reload_import_handle_error(tmp_path, main):

    TIMEOUT = 500
    qtbot, win = main
    editor = win.components["editor"]
    debugger = win.components["debugger"]
    traceback_view = win.components["traceback_viewer"]

    editor.autoreload(True)
    editor.preferences["Autoreload: watch imported modules"] = True

    # save the module and top level script files
    module_file = Path(tmp_path).joinpath("module_makebox.py")
    script = Path(tmp_path).joinpath("main.py")
    modify_file(code_module_makebox, module_file)
    modify_file(code_import_module_makebox, script)

    # run, verify that no exception was generated
    editor.load_from_file(script)
    debugger._actions["Run"][0].triggered.emit()
    assert(traceback_view.current_exception.text()  == "")

    # save the module with an error
    with qtbot.waitSignal(editor.triggerRerender, timeout=TIMEOUT):
        lines = code_module_makebox.splitlines()
        lines.remove("    zval = z + 1") # introduce NameError
        lines = "\n".join(lines)
        modify_file(lines, module_file)

    # verify NameError is generated
    debugger._actions["Run"][0].triggered.emit()
    assert("NameError" in traceback_view.current_exception.text())

    # revert the error, verify rerender is triggered
    with qtbot.waitSignal(editor.triggerRerender, timeout=TIMEOUT):
        modify_file(code_module_makebox, module_file)

    # verify that no exception was generated
    debugger._actions["Run"][0].triggered.emit()
    assert(traceback_view.current_exception.text()  == "")

def test_modulefinder(tmp_path, main):

    TIMEOUT = 500
    qtbot, win = main
    editor = win.components["editor"]
    debugger = win.components["debugger"]
    traceback_view = win.components["traceback_viewer"]
    log = win.components['log']

    editor.autoreload(True)
    editor.preferences["Autoreload: watch imported modules"] = True

    script = Path(tmp_path).joinpath("main.py")
    Path(tmp_path).joinpath("emptydir").mkdir()
    modify_file("#import emptydir", script)
    editor.load_from_file(script)
    with qtbot.waitSignal(editor.triggerRerender, timeout=TIMEOUT):
        modify_file("import emptydir", script)

    qtbot.wait(100)
    assert("Cannot determine imported modules" in log.toPlainText().splitlines()[-1])

