from src.main import MainWindow
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