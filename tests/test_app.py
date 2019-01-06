import os.path as path
import os
import tempfile

from PyQt5.QtCore import Qt
from PyQt5.QtWidgets import QFileDialog

from src.main import MainWindow
from src.widgets.editor import Editor
import pytest
import pytestqt

@pytest.fixture
def main(qtbot):
    
    win = MainWindow()
    win.show()
    
    qtbot.addWidget(win)
    
    debugger = win.components['debugger']
    debugger._actions['Run'][0].triggered.emit()
    
    return qtbot, win

def test_render(main):
    
    qtbot, win = main
    
    obj_tree_comp = win.components['object_tree']
    
    assert(obj_tree_comp.CQ.childCount() == 1)
    
    obj_tree_comp._toolbar_actions[0].triggered.emit()
    
    assert(obj_tree_comp.CQ.childCount() == 0)
    
def test_export(main,mock):
    
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
    mock.patch.object(QFileDialog, 'getSaveFileName', return_value=('out.stl',''))
    obj_tree_comp._export_STL_action.triggered.emit()
    assert(path.isfile('out.stl'))
    
    #export STEP
    mock.patch.object(QFileDialog, 'getSaveFileName', return_value=('out.step',''))
    obj_tree_comp._export_STEP_action.triggered.emit()
    assert(path.isfile('out.step'))
    
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

code = \
'''import cadquery as cq
result = cq.Workplane("XY" )
result = result.box(3, 3, 0.5)
result = result.edges("|Z").fillet(0.125)'''
    
def test_debug(main,mock):
    
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
    
@pytest.fixture
def editor(qtbot):
    
    win = Editor()
    win.show()
    
    qtbot.addWidget(win)
    
    return qtbot, win

def test_editor(monkeypatch,editor):
    
    qtbot, editor = editor
    
    with open('test.py','w') as f:
        f.write(code)
    
    #check that no text is present
    assert(editor.get_text_with_eol() == '')
    
    #check that loading from file works properly
    editor.load_from_file('test.py')
    assert(len(editor.get_text_with_eol()) > 0)
    assert(editor.get_text_with_eol() == code)
    
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
    assert(editor.get_text_with_eol() == code)
    
    #check that save file works properly
    editor.set_text('a')
    editor._filename = 'test2.py'
    editor.save()
    
    monkeypatch.setattr(QFileDialog, 'getOpenFileName', 
                        staticmethod(filename2))
                        
    editor.open()
    assert(editor.get_text_with_eol() == 'a')
    assert(editor.get_text_with_eol() == 'a')
    
    #check that save as works properly
    os.remove('test2.py')
    editor.save_as()
    assert(os.path.exists(filename2()[0]))

def test_editor_autoreload(monkeypatch,editor):
    qtbot, editor = editor

    # start out with autoreload enabled
    editor.autoreload(True)

    with open('test.py','w') as f:
        f.write(code)

    assert(editor.get_text_with_eol() == '')

    editor.load_from_file('test.py')
    assert(len(editor.get_text_with_eol()) > 0)

    # wait for reload.
    with qtbot.waitSignal(editor.triggerRerender, timeout=500):
        # modify file
        with open('test.py', 'w') as f:
            f.write('new_model = cq.Workplane("XY").box(1,1,1)\n')

    # check that editor has updated file contents
    assert("new_model" in editor.get_text_with_eol())

    # disable autoreload
    editor.autoreload(False)

    # Wait for reload in case it incorrectly happens. A timeout should occur
    # instead because a re-render should not be triggered with autoreload
    # disabled.
    with pytest.raises(pytestqt.exceptions.TimeoutError):
        with qtbot.waitSignal(editor.triggerRerender, timeout=500):
            # re-write original file contents
            with open('test.py','w') as f:
                f.write(code)

    # editor should continue showing old contents since autoreload is disabled.
    assert("new_model" in editor.get_text_with_eol())

    # Saving a file with autoreload disabled should not trigger a rerender.
    with pytest.raises(pytestqt.exceptions.TimeoutError):
        with qtbot.waitSignal(editor.triggerRerender, timeout=500):
            editor.save()

    editor.autoreload(True)

    # Saving a file with autoreload enabled should trigger a rerender.
    with qtbot.waitSignal(editor.triggerRerender, timeout=500):
        editor.save()
