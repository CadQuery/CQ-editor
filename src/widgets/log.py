import logbook as logging

from PyQt5.QtWidgets import QPlainTextEdit
from PyQt5 import QtCore

from ..mixins import ComponentMixin

class QtLogHandler(logging.Handler,logging.StringFormatterHandlerMixin):
    
    def __init__(self, log_widget,*args,**kwargs):
        
        super(QtLogHandler,self).__init__(*args,**kwargs)
        logging.StringFormatterHandlerMixin.__init__(self,None)
        
        self.log_widget = log_widget

    def emit(self, record):
        
        msg = self.format(record)
        QtCore.QMetaObject\
            .invokeMethod(self.log_widget,
                          'appendPlainText',
                          QtCore.Qt.QueuedConnection,
                          QtCore.Q_ARG(str, msg))

class LogViewer(QPlainTextEdit, ComponentMixin):
    
    name = 'Log viewer'
    
    def __init__(self,*args,**kwargs):
        
        super(LogViewer,self).__init__(*args,**kwargs)
        self._MAX_ROWS = 500
        
        self.setReadOnly(True)
        self.setMaximumBlockCount(self._MAX_ROWS)
        self.setLineWrapMode(QPlainTextEdit.NoWrap)
        
        self.handler = QtLogHandler(self)
        
    def append(self,msg):
        
        self.appendPlainText(msg)