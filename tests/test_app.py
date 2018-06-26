import os.path as path
import os

from PyQt5.QtCore import Qt
from PyQt5.QtWidgets import QFileDialog

from src.main import MainWindow
from src.widgets.editor import Editor
import pytest

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
    
    obj_tree = win.components['object_tree']
    
    assert(obj_tree.CQ.childCount() == 1)
    
    obj_tree._toolbar_actions[0].triggered.emit()
    
    assert(obj_tree.CQ.childCount() == 0)
    
def test_export(main,mock):
    
    qtbot, win = main
    
    debugger = win.components['debugger']
    debugger._actions['Run'][0].triggered.emit()
       
    #set focus
    obj_tree = win.components['object_tree']
    qtbot.mouseClick(obj_tree, Qt.LeftButton)
    qtbot.keyClick(obj_tree, Qt.Key_Down)
    qtbot.keyClick(obj_tree, Qt.Key_Down)
    
    #export STL
    mock.patch.object(QFileDialog, 'getSaveFileName', return_value=('out.stl',''))
    obj_tree._export_STL_action.triggered.emit()
    assert(path.isfile('out.stl'))
    
    #export STEP
    mock.patch.object(QFileDialog, 'getSaveFileName', return_value=('out.step',''))
    obj_tree._export_STEP_action.triggered.emit()
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
    obj_tree = win.components['object_tree']
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
    
@pytest.fixture
def editor(qtbot):
    
    win = Editor()
    win.show()
    
    qtbot.addWidget(win)
    
    return qtbot, win

def test_editor(editor):
    
    pass