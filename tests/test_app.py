from path import Path
import os, sys

from multiprocessing import Process

import pytest
import pytestqt
import cadquery as cq

from PyQt5.QtCore import Qt, QSettings
from PyQt5.QtWidgets import QFileDialog, QMessageBox, QApplication

from cq_editor.__main__ import MainWindow
from cq_editor.widgets.editor import Editor
from cq_editor.cq_utils import export

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

debug('test')
show_object(result,name='test')
'''

code_show_Shape = \
'''import cadquery as cq
result = cq.Workplane("XY" )
result = result.box(3, 3, 0.5)
result = result.edges("|Z").fillet(0.125)

show_object(result.val())
'''

code_multi = \
'''import cadquery as cq
result1 = cq.Workplane("XY" ).box(3, 3, 0.5)
result2 = cq.Workplane("XY" ).box(3, 3, 0.5).translate((0,15,0))
'''

def _modify_file(code):
        with open('test.py', 'w', 1) as f:
                    f.write(code)

def modify_file(code):
    p = Process(target=_modify_file,args=(code,))
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

    from OCC.AIS import AIS_ListOfInteractive
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


def test_debug(main,mocker):

    # store the tracing function
    trace_function = sys.gettrace()

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

    def assert_func(x):
        '''Neddedd to perform asserts in lambdas
        '''
        assert(x)

    def patch_debugger(debugger,event_loop_mock):

        debugger.inner_event_loop.exec_ = event_loop_mock.exec_
        debugger.inner_event_loop.exit = event_loop_mock.exit


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

    viewer = win.components['viewer']
    assert(number_visible_items(viewer) == 3)

    #test step through
    ev = event_loop([lambda: (assert_func(variables.model().rowCount() == 0),
                              assert_func(number_visible_items(viewer) == 3),
                              step.triggered.emit()),
                     lambda: (assert_func(variables.model().rowCount() == 1),
                              assert_func(number_visible_items(viewer) == 3),
                              step.triggered.emit()),
                     lambda: (assert_func(variables.model().rowCount() == 2),
                              assert_func(number_visible_items(viewer) == 3),
                              step.triggered.emit()),
                     lambda: (assert_func(variables.model().rowCount() == 2),
                              assert_func(number_visible_items(viewer) == 4),
                              cont.triggered.emit())])

    patch_debugger(debugger,ev)

    debug.triggered.emit(True)
    assert(variables.model().rowCount() == 2)
    assert(number_visible_items(viewer) == 3)

    #test exit debug
    ev = event_loop([lambda: (step.triggered.emit(),),
                     lambda: (assert_func(variables.model().rowCount() == 1),
                              assert_func(number_visible_items(viewer) == 3),
                              debug.triggered.emit(False),)])

    patch_debugger(debugger,ev)

    debug.triggered.emit(True)

    assert(variables.model().rowCount() == 1)
    assert(number_visible_items(viewer) == 3)

    #test breakpoint
    ev = event_loop([lambda: (cont.triggered.emit(),),
                     lambda: (assert_func(variables.model().rowCount() == 2),
                              assert_func(number_visible_items(viewer) == 4),
                              cont.triggered.emit(),)])

    patch_debugger(debugger,ev)

    editor.set_breakpoints([(4,None)])

    debug.triggered.emit(True)

    assert(variables.model().rowCount() == 2)
    assert(number_visible_items(viewer) == 3)

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

    # restore the tracing function
    sys.settrace(trace_function)

@pytest.fixture
def editor(qtbot):

    win = Editor()
    win.show()

    qtbot.addWidget(win)

    return qtbot, win

def test_editor(monkeypatch,editor):

    def conv_line_ends(text):
        return '\n'.join(text.splitlines())

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

    view = viewer.canvas._display.GetView()
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
    editor = win.components['editor']
    debugger = win.components['debugger']

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
