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
    
    return qtbot, win

def test_render(main):
    
    qtbot, win = main
    
    editor = win.components['editor']
    editor._actions['Run'][0].triggered.emit()
    
    obj_tree = win.components['object_tree']
    
    assert(obj_tree.CQ.childCount() == 1)
    
    obj_tree._toolbar_actions[0].triggered.emit()
    
    assert(obj_tree.CQ.childCount() == 0)
    
def test_export(main,mock):
    
    qtbot, win = main
    
    editor = win.components['editor']
    editor._actions['Run'][0].triggered.emit()
    
    obj_tree = win.components['object_tree']
    qtbot.addWidget(obj_tree)
    
    #set focus
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
    
@pytest.fixture
def editor(qtbot):
    
    win = Editor()
    win.show()
    
    qtbot.addWidget(win)
    
    return qtbot, win

def test_editor(editor):
    
    pass