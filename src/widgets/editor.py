from spyder.widgets.sourcecode.codeeditor import  CodeEditor
from PyQt5.QtCore import pyqtSignal
from PyQt5.QtWidgets import QAction, QFileDialog

import cadquery as cq
import imp
import qtawesome as qta
import sys


from ..mixins import ComponentMixin
from ..icons import icon

class Editor(CodeEditor,ComponentMixin):
    
    EXTENSIONS = '*.py'

    sigRendered = pyqtSignal(list)
    sigTraceback = pyqtSignal(object,str)
    
    def __init__(self,parent=None):
        
        super(Editor,self).__init__(parent)
        
        self._filename = ''
        
        self.setup_editor(linenumbers=True,
                          markers=True,
                          edge_line=False,
                          tab_mode=False,
                          show_blanks=True,
                          language='Python')
        
        self._actions =  \
                {'File' : [QAction(icon('new'),
                                  'New',
                                  self,triggered=self.new),
                          QAction(icon('open'),
                                  'Open',
                                  self,triggered=self.open),
                          QAction(icon('save'),
                                  'Save',
                                  self,triggered=self.save),
                          QAction(icon('save_as'),
                                  'Save as',
                                  self,triggered=self.save_as)],
                'Run' : [QAction(icon('run'),
                                 'Render',
                                 self,triggered=self.render)]}
        
        for a in self._actions.values():
            self.addActions(a)
    
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
            cq_code = compile(cq_script,'<string>','exec')
            exec(cq_code,t.__dict__,t.__dict__)
            results = t.__dict__
            cq_objects = [(k,v.val().wrapped) for k,v in results.items() if isinstance(v,cq.Workplane)]
            self.sigRendered.emit(cq_objects)
            self.sigTraceback.emit(None,
                                   cq_script)
        except Exception: 
            self.sigTraceback.emit(sys.exc_info(),
                                   cq_script)
        
if __name__ == "__main__":
    
    from PyQt5.QtWidgets import QApplication
    
    app = QApplication(sys.argv)
    editor = Editor()
    editor.show()
    
    sys.exit(app.exec_())
        