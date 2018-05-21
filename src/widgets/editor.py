from spyder.widgets.sourcecode.codeeditor import  CodeEditor
from PyQt5.QtCore import pyqtSignal
from PyQt5.QtWidgets import QAction, QFileDialog

import cadquery as cq
import imp
import qtawesome as qta

class Editor(CodeEditor):
    
    EXTENSIONS = '*.py'

    sigRendered = pyqtSignal(list)
    
    def __init__(self,parent=None):
        
        super(Editor,self).__init__(parent)
        
        self._filename = ''
        
        self.setup_editor(linenumbers=True,
                          markers=True,
                          edge_line=False,
                          tab_mode=False,
                          show_blanks=True,
                          language='Python')
        
        self.addAction(QAction(qta.icon('fa.file'),'New',self,triggered=self.new))
        self.addAction(QAction(qta.icon('fa.folder-open'),'Open',self,triggered=self.open))
        self.addAction(QAction(qta.icon('fa.save'),'Save',self,triggered=self.save))
        self.addAction(QAction(qta.icon('fa.save','fa.pencil'),'Save as',self,triggered=self.save_as))
        self.addAction(QAction(qta.icon('fa.play'),'Render',self,triggered=self.render))
    
    def new(self):
        
        self._filename = ''
        self.set_text('')

    def open(self):
        
        fname,_ = QFileDialog.getOpenFileName(self,filter=self.EXTENSIONS)
        if fname is not '':
            self.set_text_from_file(fname)
            self._filename = fname
    
    def save(self):
        
        if self._filename is not '':
            with open(self._filename,'w') as f:
                f.write(self.get_text_with_eol())
        else:
            self.save_as()
        
    def save_as(self):
        
        fname,_ = QFileDialog.getSaveFileName(self,filter=self.EXTENSIONS)
        if fname is not '':
             with open(self._filename,'w') as f:
                f.write(self.get_text_with_eol())
                self._filename = fname
                
    def render(self):
               
        cq_script = self.get_text_with_eol()
        results = {}
        
        try:
            t = imp.new_module('temp')        
            exec(cq_script,t.__dict__,results)
            cq_objects = [(k,v.val().wrapped) for k,v in results.items() if isinstance(v,cq.Workplane)]
            self.sigRendered.emit(cq_objects)
        except Exception: 
            pass #fixme: add logging
        
if __name__ == "__main__":
    
    import sys
    from PyQt5.QtWidgets import QApplication
    
    app = QApplication(sys.argv)
    editor = Editor()
    editor.show()
    
    sys.exit(app.exec_())
        