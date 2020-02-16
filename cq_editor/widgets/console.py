from PyQt5.QtWidgets import QApplication
from PyQt5.QtCore import pyqtSlot

from qtconsole.rich_jupyter_widget import RichJupyterWidget
from qtconsole.inprocess import QtInProcessKernelManager

from ..mixins import ComponentMixin

class ConsoleWidget(RichJupyterWidget,ComponentMixin):
    
    name = 'Console'

    def __init__(self, customBanner=None, namespace=dict(), *args, **kwargs):
        super(ConsoleWidget, self).__init__(*args, **kwargs)

#        if not customBanner is None:
#            self.banner = customBanner

        self.font_size = 6
        self.kernel_manager = kernel_manager = QtInProcessKernelManager()
        kernel_manager.start_kernel(show_banner=False)
        kernel_manager.kernel.gui = 'qt'
        kernel_manager.kernel.shell.banner1 = ""
        
        self.kernel_client = kernel_client = self._kernel_manager.client()
        kernel_client.start_channels()

        def stop():
            kernel_client.stop_channels()
            kernel_manager.shutdown_kernel()
            QApplication.instance().exit()

        self.exit_requested.connect(stop)
        
        self.clear()
        
        self.push_vars(namespace)

    @pyqtSlot(dict)
    def push_vars(self, variableDict):
        """
        Given a dictionary containing name / value pairs, push those variables
        to the Jupyter console widget
        """
        self.kernel_manager.kernel.shell.push(variableDict)

    def clear(self):
        """
        Clears the terminal
        """
        self._control.clear()


    def print_text(self, text):
        """
        Prints some plain text to the console
        """
        self._append_plain_text(text)

    def execute_command(self, command):
        """
        Execute a command in the frame of the console widget
        """
        self._execute(command, False)
        
    def _banner_default(self):
        
        return ''

        
if __name__ == "__main__":
   
    
    import sys
    
    app = QApplication(sys.argv)
    
    console = ConsoleWidget(customBanner='IPython console test')
    console.show()
    
    sys.exit(app.exec_())
