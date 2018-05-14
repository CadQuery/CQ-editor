from spyder.widgets.sourcecode.codeeditor import  CodeEditor
from PyQt5.QtCore import pyqtSignal
from PyQt5.QtWidgets import QAction

import cadquery as cq
import imp
import qtawesome as qta

class Editor(CodeEditor):

    sigRendered = pyqtSignal(list)
    
    def __init__(self,parent=None):
        
        super(Editor,self).__init__(parent)
        
        self.setup_editor(linenumbers=True,
                          markers=True,
                          edge_line=False,
                          tab_mode=False,
                          show_blanks=True,
                          language='Python')
        
        self.addAction(QAction(qta.icon('fa.play'),'Render',self,triggered=self.render))

        
    def render(self):
               
        cq_script = self.get_text_with_eol()
        results = {}
        
        try:
            t = imp.new_module('temp')        
            exec(cq_script,t.__dict__,results)
            cq_objects = [v.val() for v in results.values() if isinstance(v,cq.Workplane)]
        except Exception: #fixme: add logging
            pass

        self.sigRendered.emit(cq_objects)
        
        
if __name__ == "__main__":
    
    import sys
    from PyQt5.QtWidgets import QApplication
    
    app = QApplication(sys.argv)
    editor = Editor()
    editor.show()
    
    sys.exit(app.exec_())
        