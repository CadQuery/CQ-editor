# -*- coding: utf-8 -*-
"""
Created on Wed Jun 28 11:10:59 2017
141
@author: aurbancz
"""

import numpy as np
import pandas as pd

from PyQt5 import QtGui, QtCore, QtWidgets, QtWebKitWidgets
from PyQt5.QtCore import QAbstractListModel, QMimeData, QModelIndex, Qt

from matplotlib.backends.backend_qt5agg import FigureCanvasQTAgg as FigureCanvas, NavigationToolbar2QT as NavigationToolbar
from matplotlib.figure import Figure

import matplotlib.patches as patches
from matplotlib.collections import PatchCollection

from layouts import vbox, vspacer, action

from collections import OrderedDict

from future.utils import listvalues

from logbook import Logger, StreamHandler

log = Logger('widgets')

SMALL_LW = 1
LARGE_LW = 2
ZORDER_PATCH = 1
ZORDER_DECORATION = 2
WAFER_EDGE_COLOR = 'k'

def rect(x,
         y,
         sizex=1,
         sizey=1,
         color=False):
    
    return patches.Rectangle((x-sizex/2,y-sizey/2),
                             sizex,
                             sizey,
                             fill=color,
                             zorder=ZORDER_PATCH)

def notch_profile(t,
                  a,
                  width,
                  height):
    
    rv = abs(t-a)-width
    rv2 = rv*(rv<0)
    
    return -rv2/rv2.min()*height

def wafer_edge(r=.15,
               fec=.03,
               N=400,
               notch=.03):
    
    t = np.linspace(0,2*np.pi,N)
    r = r + notch_profile(t,np.pi,0.02,0.005)
    
    return r*np.sin(t),r*np.cos(t)

#==============================================================================
# Rudimentary widgets
#==============================================================================

from layouts import hbox

class HtmlView(QtWebKitWidgets.QWebView):
    
    def __init__(self,parent=None):
        
        super(HtmlView,self).__init__(parent=parent)
        sizePolicy = QtWidgets.QSizePolicy(QtWidgets.QSizePolicy.Expanding,
                                           QtWidgets.QSizePolicy.Expanding)
        self.setSizePolicy(sizePolicy)
        
        self._body = self.page().mainFrame().findFirstElement('body')
        self._body.appendInside(self._stylesheet())
        
        self.setContextMenuPolicy(0) #disable context menu
        
    def append(self,s):
        
        self._body.appendInside(s)
        
    def _stylesheet(self):
        
        return '''<style type="text/css">
                  table, th, td {border: 1px solid black;
                                 border-collapse: collapse;}
                  
                  body {font-family: sans-serif;}

                  </style>'''
    
    def clear(self):
        
        self._body.setInnerXml(self._stylesheet())

class FileField(QtWidgets.QWidget):
    
    value_changed = QtCore.pyqtSignal(str)

    def __init__(self, value = '',pattern ='*.*', parent=None):
        
        super(FileField,self).__init__(parent=parent)
        self.value = value
        self.pattern = pattern
        
        self.lineedit = QtWidgets.QLineEdit(value, parent)
        self.btn = QtWidgets.QPushButton(self.style().standardIcon(QtWidgets.QStyle.SP_DirIcon),
                                         '')
        
        self.btn.clicked.connect(self.get_value)
        
        hbox((self.lineedit,self.btn),parent=self)

    @QtCore.pyqtSlot()
    def get_value(self):
        
        name, _filter = QtWidgets.QFileDialog.getOpenFileName(None, 'Select file',
                                                              filter=self.pattern)

        if name:
            self.lineedit.setText(name)
            self.value_changed.emit(name)

    def text(self):
        return self.lineedit.text()
    

class DirField(FileField):
    
    
    @QtCore.pyqtSlot()
    def get_value(self):
        
        name = QtWidgets.QFileDialog.getExistingDirectory(None,
                                                          'Select directory')
        
        if name:
            self.lineedit.setText(name)
            self.value_changed.emit(name)


class StatusIndicator(QtWidgets.QLabel):
    
    state_mapping = {False: 0,
                     True:  1,
                     None:  2}
    
    def __init__(self,
                 texts=('<center>NOK</center>',
                        '<center>OK</center>',
                        '<center>?</center>'), 
                 colors=('#FF6347','#00FF7F','lightgray'),
                 parent=None):
        
        super(StatusIndicator,self).__init__(parent=parent)
        
        self.texts = texts
        self.colors = colors
        
        self.set_state(None)
    
    @QtCore.pyqtSlot(object)
    def set_state(self,value):
        
        ix = self.state_mapping[value]
        self.setText(self.texts[ix])
        self.set_color(self.colors[ix])
        
    def set_color(self,color):
        
        self.setStyleSheet('background: {}'.format(color))


class TitleLabel(QtWidgets.QLabel):
    
    def __init__(self,text='',parent=None):
        
        super(TitleLabel,self).__init__(parent=parent)                      

        self.setText('<h3>{}</h3>'.format(text))
        
class FingerTabBarWidget(QtWidgets.QTabBar):
    
    def __init__(self, parent=None, *args, **kwargs):
        self.tabSize = QtCore.QSize(kwargs.pop('width',100), kwargs.pop('height',25))
        
        super(FingerTabBarWidget,self).__init__(parent, *args, **kwargs)
                 
    def paintEvent(self, event):
        painter = QtGui.QStylePainter(self)
        option = QtGui.QStyleOptionTab()
 
        for index in range(self.count()):
            self.initStyleOption(option, index)
            tabRect = self.tabRect(index)
            tabRect.moveLeft(10)
            painter.drawControl(QtGui.QStyle.CE_TabBarTabShape, option)
            painter.drawText(tabRect, QtCore.Qt.AlignVCenter |\
                             QtCore.Qt.TextDontClip, \
                             self.tabText(index));
        painter.end()
        
    def tabSizeHint(self,index):
        return self.tabSize

class FingerTabWidget(QtWidgets.QTabWidget):
    
    def __init__(self, parent, *args):
        
        super(FingerTabWidget,self).__init__(self, parent, *args)
        self.setTabBar(FingerTabBarWidget(self))
        
class DataFrameColumnSelector(QtWidgets.QListView):
    
    selection_changed = QtCore.pyqtSignal(list)
    
    def __init__(self,
                 df=None,
                 exclusive=False,
                 mask=[],
                 parent=None):
        
        super(DataFrameColumnSelector,self).__init__(parent=parent)
        
        self._model = QtGui.QStandardItemModel(parent = self)
        self._exclusive = exclusive
        self._mask = mask
        
        if not df is None:
            self.update_items(df)

    @QtCore.pyqtSlot(pd.DataFrame)
    def update_items(self,df):
        
        self._model.clear()
        self._columns = df.columns.difference(self._mask)
        
        for col in self._columns:
            item = QtGui.QStandardItem(str(col))
            item.setCheckable(True)
            
            self._model.appendRow(item)
            
        self._selection = np.array([False,]*len(self._columns),dtype=bool)
        self.setModel(self._model)
        
        if self._exclusive:
            self._selection[-1] = True
            item.setCheckState(Qt.Checked)
            
        self.selection_changed.emit(self._columns[self._selection].tolist())
        self._model.itemChanged.connect(self.check_selection)
        
    @QtCore.pyqtSlot(QtGui.QStandardItem)
    def check_selection(self,item):
        
        if self._exclusive:
            self._selection[:] = False

            #uncheck all items            
            old_state = self._model.blockSignals(True)
            for i in range(self._model.rowCount()):
                if  i != item.row():
                    self._model.item(i).setCheckState(Qt.Unchecked)
            self._model.blockSignals(old_state)
        
        self._selection[item.row()] ^= True
        self.selection_changed.emit(self._columns[self._selection].tolist())
    
#==============================================================================
# Embedded console widget
#==============================================================================

from qtconsole.rich_jupyter_widget import RichJupyterWidget
from qtconsole.inprocess import QtInProcessKernelManager


class ConsoleWidget(RichJupyterWidget):

    def __init__(self, customBanner=None, namespace=dict(), *args, **kwargs):
        super(ConsoleWidget, self).__init__(*args, **kwargs)

#        if not customBanner is None:
#            self.banner = customBanner

        self.font_size = 6
        self.kernel_manager = kernel_manager = QtInProcessKernelManager()
        kernel_manager.start_kernel(show_banner=False)
        kernel_manager.kernel.gui = 'qt'
        self.kernel_client = kernel_client = self._kernel_manager.client()
        kernel_client.start_channels()

        def stop():
            kernel_client.stop_channels()
            kernel_manager.shutdown_kernel()
            QtGui.QApplication.instance().exit()

        self.exit_requested.connect(stop)
        
        self.clear()
        
        self.push_vars(namespace)

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

#==============================================================================
# Matplotlib related widgets
#==============================================================================

class MatplotlibWidget(FigureCanvas):
    
    signal_data_changed = QtCore.pyqtSignal()
    
    def __init__(self,
                 parent=None,
                 w=5,
                 h=5,
                 dpi=70,
                 xlim=(-0.16,0.16),
                 ylim=(-0.16,0.16),
                 aspect='equal'):
    
        #matplotlib related
        self.figure = Figure(figsize=(w,h),dpi=dpi)
        self._axis = self.figure.add_subplot(111,aspect=aspect)
        self._axis.set_xlim(*xlim)
        self._axis.set_ylim(*ylim)
        
        #call superclass and set parent
        super(MatplotlibWidget,self).__init__(self.figure)
        self.setParent(parent)
        
        #set size policy (PyQt stuff)
        self.size_policy()
        super(MatplotlibWidget,self).updateGeometry() 
        
        #run all hooks
        self.prepare_axis()
        self.add_widgets()
        
    def size_policy(self):
        
        sizePolicy = QtWidgets.QSizePolicy(QtWidgets.QSizePolicy.Preferred,
                                           QtWidgets.QSizePolicy.Preferred)
        sizePolicy.setHeightForWidth(True)
        self.setSizePolicy(sizePolicy)
        '''
        sizePolicy = QtWidgets.QSizePolicy(QtWidgets.QSizePolicy.Preferred,
                                           QtWidgets.QSizePolicy.Preferred)
        #sizePolicy.setHeightForWidth(True)
        sizePolicy.setWidthForHeight(True)
        self.setSizePolicy(sizePolicy)
        '''
        
    def heightForWidth(self, width):
        
        return width
    
    def widthForHeight(self, width):

        return width

    def prepare_axis(self):
        
        pass
    
    def add_widgets(self):
        
        pass
    
    @QtCore.pyqtProperty(object)
    def ax(self):
        
        return self._axis
    
    @QtCore.pyqtProperty(object,notify=signal_data_changed)
    def data(self):
        
        return self._data
    
    @data.setter
    def data(self,data):
        
        self._data = data
        self._update_plot()
        
    def _update_plot(self):
        '''
        Hook called after data changed
        '''
        
        pass
    
class IntrafieldWidget(MatplotlibWidget):
    '''
    Matplotlib based widget for intrafield plotting
    '''
    
    signal_item_selected = QtCore.pyqtSignal(int)
    facecolor = 'w'
    edgecolor = 'k'
    
    def prepare_axis(self):
        
        self._axis.xaxis.set_visible(False)
        self._axis.yaxis.set_visible(False)
        
        self._pc = None
        
    def _update_plot(self):
        
        #make the patches
        self._patches = [rect(x,y,sx,sy) for x,y,sx,sy in self._data]
        
        #generate selection mask
        self._selection = np.zeros(self._data.shape[0],dtype=bool)
        
        #generate linewidths array
        self._linewidths = np.ones_like(self._selection,dtype=float)
        
        if self._pc:
            self._pc.remove()
        
        self._pc = PatchCollection(self._patches,
                                   picker=1,
                                   edgecolor=self.edgecolor,
                                   facecolor=self.facecolor,
                                   linewidth = self._linewidths)
        self._axis.add_collection(self._pc)
        
        self.mpl_connect('pick_event', self.item_selected)
        
    def item_selected(self,event):

        ind = event.ind[0]
        self.toggle_item(ind)
        self.signal_item_selected.emit(ind)
    
    def toggle_item(self,ind):
        self._selection[ind] ^= True
        
        self._linewidths[:] = SMALL_LW
        self._linewidths[self._selection] = LARGE_LW
        
        self._pc.set_linewidths(self._linewidths)
        
        self.draw()
    
    def select_item(self,ind):
        self._selection[:] &= False
        self._selection[ind] ^= True
        
        self._linewidths[:] = SMALL_LW
        self._linewidths[self._selection] = LARGE_LW
        
        self._pc.set_linewidths(self._linewidths)
        
        self.draw()
        
    def updateColors(self,values):
        
        self._pc.set_array(values)


class WaferWidget(IntrafieldWidget):
    '''
    Matplotlib based widget for interfield plotting. Includes wafer edge.
    '''
    
    def prepare_axis(self):
        
        super(WaferWidget,self).prepare_axis()
        self._wafer_decoration = self._axis.plot(*wafer_edge(),
                                                 zorder=ZORDER_DECORATION,
                                                 color=WAFER_EDGE_COLOR)[0]
        
        #hide spines
        for spine in self._axis.spines.values(): spine.set_visible(False)
        
        #remove margins
        self.figure.subplots_adjust(left=0,right=1,bottom=0,top=1)
        
class GuardedTransition(QtCore.QSignalTransition):
    
    def __init__(self,sig,guard):
        
        super(GuardedTransition,self).__init__(sig)
        self._guard = guard
        
    def eventTest(self,event):
        
        if self._guard(*event.arguments()):
            return True
        
        return False
    
        
class WaferCyclerWidget(WaferWidget):
    
    edgecolor = 'none'
    marker = 's'
    marker_size = 150
    
    signal_wafer_changed = QtCore.pyqtSignal(int)
    
    def __init__(self,excluded_columns=[],**kwargs):
        
        super(WaferCyclerWidget,self).__init__(**kwargs)
        self._excluded_columns = excluded_columns
        self._autoscale = True
    
    def add_widgets(self):
        
        self._wafers = []
        self._current_wafer = 0
        self._current_field = 0
        
        #prepare toolbar
        self.toolbar_widget = QtWidgets.QToolBar(parent=self)
        self.toolbar_widget.addAction('<',self.previous_wafer)
        self.toolbar_widget.addAction('>',self.next_wafer)
        self.toolbar_widget.addAction(action('a',
                                             self.toolbar_widget,
                                             self.autoscale,
                                             checkable=True))
        self.toolbar_widget.addSeparator()
        
        #store the previous and next action
        (self.previous_action,
         self.next_action) = self.toolbar_widget.actions()[:2]
        
        #combo boxes
        self.wafer_combo_widget = QtWidgets.QComboBox(sizeAdjustPolicy=QtWidgets.QComboBox.AdjustToContents)
        self.field_combo_widget = QtWidgets.QComboBox(sizeAdjustPolicy=QtWidgets.QComboBox.AdjustToContents)
        
        #add to the toolbar
        self.toolbar_widget.addWidget(self.wafer_combo_widget)
        self.toolbar_widget.addWidget(self.field_combo_widget)
        
        self.wafer_combo_widget.currentIndexChanged.connect(self.select_wafer)
        self.field_combo_widget.currentIndexChanged.connect(self.select_field)
        
        vbox((self.toolbar_widget,),
             parent=self,
             spacer=vspacer())
        
        #setup the state machine
        self.state_machine = QtCore.QStateMachine(self)
        
    @QtCore.pyqtSlot()
    def next_wafer(self):
        
        self.select_wafer(self._current_wafer+1)  
    
    @QtCore.pyqtSlot()
    def previous_wafer(self):
        
        self.select_wafer(self._current_wafer-1)
    
    @QtCore.pyqtSlot(int)
    def select_wafer(self,ix):
        
        #try:
            ix_mapped  = self._wafers[ix] #analysis:ignore
            df_wafer = self._df.query('wafer == @ix_mapped')
            data = df_wafer.loc[:,['x','y']].values.astype(float),\
                   df_wafer.loc[:,[self._current_field,]]
            self.data = np.hstack(data)
            
            if ix <= 0:
                self.previous_action.setEnabled(False)
            else:
                self.previous_action.setEnabled(True)
            
            if ix >= len(self._wafers)-1:
                self.next_action.setEnabled(False)
            else:
                self.next_action.setEnabled(True)
            
            self._current_wafer = ix
            
            old_state = self.blockSignals(True)
            self.wafer_combo_widget.setCurrentIndex(ix)
            self.blockSignals(old_state)
            
            self.signal_wafer_changed.emit(ix)
        
#        except (IndexError,ValueError) as e:
#            log.error(e)
            
    @QtCore.pyqtSlot(int)
    def select_field(self,ix):
        
        self._current_field = listvalues(self._fields_mapping)[ix]
        self.select_wafer(self._current_wafer)
    
    @QtCore.pyqtSlot(object)
    def set_data(self,df):
        
        wafers = df.wafer.unique()
        self._fields = fields = df.columns.difference(self._excluded_columns)
        self._fields_mapping = OrderedDict({str(f) : f for f in fields})
        
        self._df = df
        self._wafers = wafers
        self._current_wafer = 0
        self._current_field = fields[0]        
        
        self.blit
        self.setUpdatesEnabled(False)
        self.wafer_combo_widget.clear()
        self.wafer_combo_widget.addItems(map(str,range(len(wafers))))
        
        self.field_combo_widget.clear()
        self.field_combo_widget.addItems(self._fields_mapping.keys())
        self.setUpdatesEnabled(True)
        
    @QtCore.pyqtSlot(bool)
    def autoscale(self,flag):
        
        self._autoscale = flag
        if flag: self._update_plot()    
        
    def _update_plot(self):    
        
        x,y,c = self.data.T
        
        if self._pc:
            pc = self._pc
            pc.set_offsets(np.vstack((x,y)).T)
            pc.set_array(c)
            if self._autoscale:
                pc.set_clim(self._pc.set_clim(np.nanmin(c),np.nanmax(c)))
        
        else:
            self._pc = self._axis.scatter(x,
                                          y,
                                          c=c,
                                          edgecolors=self.edgecolor,
                                          marker=self.marker,
                                          s=self.marker_size)
            self.draw()
        
        self._axis.draw_artist(self._axis.patch)
        self._axis.draw_artist(self._pc)
        self._axis.draw_artist(self._wafer_decoration)
        self.update()
        self.flush_events()
 
        
#==============================================================================
# Tables / Models
#==============================================================================

 
class ListModel(QAbstractListModel):
    '''
    An example of list model with drag and drop
    '''
 
    Mimetype = 'application/vnd.row.list'
 
    def __init__(self, parent=None):
        super(ListModel, self).__init__(parent)
        self.__data = ['line 1', 'line 2', 'line 3', 'line 4']
 
    def data(self, index, role=Qt.DisplayRole):
        if not index.isValid():
            return None
 
        if index.row() > len(self.__data):
            return None
 
        if role == Qt.DisplayRole or role == Qt.EditRole:
            return self.__data[index.row()]
 
        return None
 
    def dropMimeData(self, data, action, row, column, parent):
        if action == Qt.IgnoreAction:
            return True
        if not data.hasFormat(self.Mimetype):
            return False
        if column > 0:
            return False
 
        strings = str(data.data(self.Mimetype)).split('\n')
        self.insertRows(row, len(strings))
        for i, text in enumerate(strings):
            self.setData(self.index(row + i, 0), text)
 
        return True
 
    def flags(self, index):
        flags = super(ListModel, self).flags(index)
 
        if index.isValid():
            flags |= Qt.ItemIsEditable
            flags |= Qt.ItemIsDragEnabled
        else:
            flags = Qt.ItemIsDropEnabled
 
        return flags
 
    def insertRows(self, row, count, parent=QModelIndex()):
 
        self.beginInsertRows(QModelIndex(), row, row + count - 1)
        self.__data[row:row] = [''] * count
        self.endInsertRows()
        return True
 
    def mimeData(self, indexes):
        sortedIndexes = sorted([index for index in indexes
            if index.isValid()], key=lambda index: index.row())
        encodedData = '\n'.join(self.data(index, Qt.DisplayRole)
                for index in sortedIndexes)
        mimeData = QMimeData()
        mimeData.setData(self.Mimetype, encodedData)
        return mimeData
 
    def mimeTypes(self):
        return [self.Mimetype]
 
    def removeRows(self, row, count, parent=QModelIndex()):
        self.beginRemoveRows(QModelIndex(), row, row + count - 1)
        del self.__data[row:row + count]
        self.endRemoveRows()
        return True
 
    def rowCount(self, parent=QModelIndex()):
        return len(self.__data)
 
    def setData(self, index, value, role=Qt.EditRole):
        if not index.isValid() or role != Qt.EditRole:
            return False
 
        self.__data[index.row()] = value
        self.dataChanged.emit(index, index)
        return True
 
    def supportedDropActions(self):
        return Qt.MoveAction


class DataFrameModel(QtCore.QAbstractTableModel):
    
    def __init__(self,df,parent=None):
        
        super(DataFrameModel,self).__init__(parent)
        self._data = df
        self.columns = {c : i for i,c in enumerate(df.columns)} 
        
    def rowCount(self, parent=None):
        return self._data.shape[0]

    def columnCount(self, parent=None):
        return self._data.shape[1]
    
    def data(self, index, role=QtCore.Qt.DisplayRole):
        if index.isValid():
            if role == QtCore.Qt.DisplayRole:
                return str(self._data.values[index.row(),index.column()])
            elif role == QtCore.Qt.EditRole:
                return float(self._data.values[index.row(),index.column()])
        
        return None

    def headerData(self, col, orientation, role):
        if orientation == QtCore.Qt.Horizontal and role == QtCore.Qt.DisplayRole:
            return str(self._data.columns[col])
        return None
    
#==============================================================================
# Higher level widgets
#==============================================================================
        
class FieldInspector(QtWidgets.QFrame):
    
    def __init__(self,*args,**kwargs):
        
        super(FieldInspector,self).__init__(*args,**kwargs)
        self.setupUI()
        
    def setupUI(self):
        
        #decoration
        self.setFrameStyle(QtWidgets.QFrame.Panel)
        self.setLineWidth(1)
        
        #prepare widgets
        self.id_widget = QtWidgets.QLineEdit(self)
        self.dose_widget = QtWidgets.QDoubleSpinBox(self)
        self.z_widget = QtWidgets.QDoubleSpinBox(self)
        self.rx_widget = QtWidgets.QDoubleSpinBox(self)
        self.ry_widget = QtWidgets.QDoubleSpinBox(self)
        
        self.layout_main = QtWidgets.QFormLayout(self)
        self.layout_main.addRow('Id',self.id_widget)
        self.layout_main.addRow('Dose',self.dose_widget)
        self.layout_main.addRow('Z',self.z_widget)
        self.layout_main.addRow('Rx',self.rx_widget)
        self.layout_main.addRow('Ry',self.ry_widget)

class WaferInspector(QtWidgets.QWidget):
    
    def __init__(self,*args,**kwargs):
        
        super(WaferInspector,self).__init__(*args,**kwargs)
        self.setupUI()
    
    def setupUI(self):
        
        #prepare the widgets
        self.wafer_plot = WaferWidget(self,xlim=(-0.16,0.16),ylim=(-0.16,0.16))
        self.field_list = QtWidgets.QListView(self)
        self.field_editor = FieldInspector(self)
        
        #prepate layouts
        self.layout_main = QtWidgets.QHBoxLayout(self)
        self.layout_main.addWidget(self.wafer_plot)
        self.layout_side = QtWidgets.QVBoxLayout(self)
        self.layout_main.addLayout(self.layout_side)
        
        self.layout_side.addWidget(QtWidgets.QLabel('<b>Fields</b>',self))
        self.layout_side.addWidget(self.field_list)
        
        self.layout_side.addWidget(QtWidgets.QLabel('<b>Selected field</b>',self))
        self.layout_side.addWidget(self.field_editor)
        
        self.layout_main.setStretch(0,10)
        self.layout_main.setStretch(1,5)
        
        self.layout_main.setContentsMargins(0,0,0,0)
        self.layout_main.setSpacing(0)
        
        self.layout_side.setContentsMargins(2,0,2,0)
        self.layout_side.setSpacing(5)
        
        #prepare mapping
        self.mapper = QtWidgets.QDataWidgetMapper()
        
        #handle interactivity
        self.wafer_plot.signal_item_selected.connect(self.selectFieldList)
        
        
    @QtCore.pyqtSlot(int)
    def selectFieldList(self,ix):
        '''
        Select field in the list view
        '''
        
        fl = self.field_list
        fl.setCurrentIndex(fl.model().index(ix,fl.modelColumn()))
        
    @QtCore.pyqtSlot(QtCore.QModelIndex)
    def selectFieldPlot(self,ix):
        '''
        Select field i nthe wafer view
        '''
        
        plot = self.wafer_plot
        plot.select_item(ix.row())      
        
    def setData(self,df):
        
        df = df.drop_duplicates(subset=['xf','yf','sizex','sizey'])
        self._model = DataFrameModel(df,self)
        self.wafer_plot.data = df[['xf','yf','sizex','sizey']].as_matrix()
        self.wafer_plot.updateColors(df.dose)
        self.wafer_plot.draw()
        
        self.field_list.setModel(self._model)
        self.field_list.setModelColumn(self._model.columns['id'])
        
        #setup the mapping
        self.mapper.setModel(self._model)
        
        self.mapper.addMapping(self.field_editor.id_widget,
                               self._model.columns['id'])
        self.mapper.addMapping(self.field_editor.dose_widget,
                               self._model.columns['dose'])
        self.mapper.addMapping(self.field_editor.z_widget,
                               self._model.columns['focus'])
        self.mapper.addMapping(self.field_editor.rx_widget,
                               self._model.columns['rx'])
        self.mapper.addMapping(self.field_editor.ry_widget,
                               self._model.columns['ry'])

        #connect selected item
        self.field_list.selectionModel().currentRowChanged.connect(self.mapper.setCurrentModelIndex)
        self.field_list.selectionModel().currentRowChanged.connect(self.selectFieldPlot)
        

from layouts import stack,hsplit,vbox

class MultiPageView(QtWidgets.QWidget):
    
    def __init__(self,
                 pages,
                 titles,
                 parent=None,
                 split_ratio=(1,10),
                 name='MultiPageView'):
        
        super(MultiPageView,self).__init__(parent=parent)
        
        self.setObjectName(name)
        
        self._item_count = len(titles)
        
        self.page_selector_widget = QtWidgets.QListWidget(parent=self)
        self.page_selector_widget.addItems(titles)
        self.page_selector_widget.setStyleSheet('''background-color: palette(window);
                                                border: none''')
        
        self.page_stack_widget = stack(pages,parent=self)
        
        self.page_selector_widget.currentRowChanged.connect(
                self.page_stack_widget.setCurrentIndex)
        
        self.splitter = hsplit((self.page_selector_widget,
                                self.page_stack_widget),
                                split_ratio,
                                parent=self,
                                name='_'.join((name,'splitter')))
        
        #self.splitter.setParent(self)
        #vbox((self.splitter,),parent=self,margin=0)
    
    @QtCore.pyqtSlot(int)
    def change_page(self,i):
        
        self.page_stack_widget.setCurrentIndex(i)
        
    def next(self):
        
        new_ix = self.page_stack_widget.currentIndex()+1
        
        if new_ix < self._item_count:
            self.page_stack_widget.setCurrentIndex(new_ix)
            self.page_selector_widget.setCurrentRow(new_ix)
    
    def previous(self):
        
        new_ix = self.page_stack_widget.currentIndex()-1
        
        if new_ix >= 0:
            self.page_stack_widget.setCurrentIndex(new_ix)
            self.page_selector_widget.setCurrentRow(new_ix)


class QtLogHandler(StreamHandler):
    
    def __init__(self, widget):
        super(QtLogHandler,self).__init__(None,bubble=False)
        self.widget = widget

    def emit(self, record):
        msg = self.format(record)
        self.widget.appendPlainText(msg)

        
if __name__ =='__main__':

    import BMMO
    df1 = BMMO.ADELler2dummy_df(r'C:\Localdata\ZimDesktopWikiPortable\Data\Wikis\PersonalWorking\OPP\PFC\AST-FOC\Data_minifem\ADELler_ABF_minfem_reworked_2017-01-31T11_11_36+01_00.xml')
    df = BMMO.ADELler2dummy_df(r'C:\Localdata\ZimDesktopWikiPortable\Data\Wikis\PersonalWorking\OPP\PFC\AST-FOC\ADELler_ffrfem_dummy_2017-05-29T10_29_44+02_00.xml',20,20)
    
    df['sizex'] = 0.025
    df['sizey'] = 0.033
    
    df1['sizex'] = 0.002
    df1['sizey'] = 0.0142
    
    df['id'] = range(len(df))
    df1['id'] = range(len(df1))
    
    w1 = WaferInspector()
    w1.setData(df1)
    
    w2 = WaferInspector()
    w2.setData(df)
    
    mpv = MultiPageView([w1,w2],['FFRFEM','miniFEM'])
    
    mpv.show()
    
    import DBF
    df = DBF.read_in_csv(r'C:\Localdata\ZimDesktopWikiPortable\Data\Wikis\PersonalWorking\OPP\PFC\AST-FOC\Readouts\Zebra_readout_monitoring\PJ_20170228_204338_20170228_214607_diagnostics\PJ_20170228_204338_20170228_214607_diagnostics.csv')
    
    wc = WaferCyclerWidget()
    wc.set_data(df)
    wc.show()