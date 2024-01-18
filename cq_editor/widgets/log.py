import logbook as logging
import sys
import re

from PyQt5.QtWidgets import QPlainTextEdit
from PyQt5 import QtCore

from ..mixins import ComponentMixin

def strip_escape_sequences(input_string):
    # Regular expression pattern to match ANSI escape codes
    escape_pattern = re.compile(r'\x1B\[[0-?]*[ -/]*[@-~]')

    # Use re.sub to replace escape codes with an empty string
    clean_string = re.sub(escape_pattern, '', input_string)

    return clean_string

class QtLogHandler(logging.Handler,logging.StringFormatterHandlerMixin):
    
    def __init__(self, log_widget,*args,**kwargs):
        
        super(QtLogHandler,self).__init__(*args,**kwargs)
        logging.StringFormatterHandlerMixin.__init__(self,None)
        
        self.log_widget = log_widget

    def emit(self, record):
        
        msg = self.format(record)

        msg = strip_escape_sequences(msg)

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
