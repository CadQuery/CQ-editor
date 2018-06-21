from PyQt5.QtWidgets import QTreeWidget, QTreeWidgetItem, QFileDialog, QAction
from PyQt5.QtCore import Qt, pyqtSlot, pyqtSignal

from OCC.AIS import AIS_ColoredShape, AIS_Line
from OCC.Quantity import Quantity_NOC_RED as RED
from OCC.Quantity import Quantity_NOC_GREEN as GREEN
from OCC.Quantity import Quantity_NOC_BLUE1 as BLUE
from OCC.Geom import Geom_CylindricalSurface, Geom_Plane, Geom_Circle,\
     Geom_TrimmedCurve, Geom_Axis1Placement, Geom_Axis2Placement, Geom_Line
from OCC.gp import gp_Trsf, gp_Vec, gp_Ax3, gp_Dir, gp_Pnt, gp_Ax1

from ..mixins import ComponentMixin
from ..icons import icon

from cadquery import Vector, exporters

class TopTreeItem(QTreeWidgetItem):
    
    def __init__(self,*args,**kwargs):
        
        super(TopTreeItem,self).__init__(*args,**kwargs)
        
class ObjectTreeItem(QTreeWidgetItem):
    
    def __init__(self,*args,ais=None,shape=None,**kwargs):
        
        super(ObjectTreeItem,self).__init__(*args,**kwargs)
        self.setFlags( self.flags() | Qt.ItemIsUserCheckable)
        self.setCheckState(0,Qt.Checked)
        
        self.ais = ais
        self.shape= shape

class CQRootItem(TopTreeItem):
    
    def __init__(self,*args,**kwargs):
        
        super(CQRootItem,self).__init__(['CQ models'],*args,**kwargs)
        
        
class ImportRootItem(TopTreeItem):
    
    def __init__(self,*args,**kwargs):
        
        super(ImportRootItem,self).__init__(['Imports'],*args,**kwargs)
        
class HelpersRootItem(TopTreeItem):
    
    def __init__(self,*args,**kwargs):
        
        super(HelpersRootItem,self).__init__(['Helpers'],*args,**kwargs)


class ObjectTree(QTreeWidget,ComponentMixin):
    
    name = 'Object Tree'
    _stash = []
    
    sigObjectsAdded = pyqtSignal(list)
    sigObjectsRemoved = pyqtSignal(list)
    sigCQObjectSelected = pyqtSignal(object)
    
    def  __init__(self,parent):
        
        super(ObjectTree,self).__init__(parent)   
        self.setHeaderHidden(True)
        self.setItemsExpandable(False)
        self.setRootIsDecorated(False)
        self.setContextMenuPolicy(Qt.ActionsContextMenu)
        
        self.CQ = CQRootItem()
        self.Imports = ImportRootItem()
        self.Helpers = HelpersRootItem()
        
        root = self.invisibleRootItem()
        root.addChild(self.CQ)
        root.addChild(self.Imports)
        root.addChild(self.Helpers)
        
        self._export_STL_action = QAction('Export as STL',
                                        self,
                                        triggered=self.exportSTL)
        
        self._toolbar_actions = \
            [QAction(icon('delete-many'),'Clear all',self,triggered=self.removeObjects),
             QAction(icon('delete'),'Clear current',self,triggered=self.removeSelected)]
        
        self.addActions(self._toolbar_actions)
        self.addAction(self._export_STL_action)
        
        self.itemSelectionChanged.connect(self.handleSelection)
        
    def menuActions(self):
        
        return {}
    
    def toolbarActions(self):
        
        return self._toolbar_actions
    
    def addLines(self):
        
        origin = (0,0,0)
        ais_list = []
        
        for name,color,direction in zip(('X','Y','Z'),
                                        (RED,GREEN,BLUE),
                                        ((1,0,0),(0,1,0),(0,0,1))):
            line_placement = Geom_Line(gp_Ax1(gp_Pnt(*origin),
                                       gp_Dir(*direction)))
            line = AIS_Line(line_placement.GetHandle())
            line.SetColor(color)
            
            self.Helpers.addChild(ObjectTreeItem([name],
                                                 ais=line))
            
            ais_list.append(line)
            
        self.sigObjectsAdded.emit(ais_list)
        self.expandToDepth(1)
    
    
    @pyqtSlot(dict,bool)
    @pyqtSlot(dict)
    def addObjects(self,objects,clean=False,root=None,alpha=0.):
        
        if clean:
            self.removeObjects()
        
        ais_list = []
        
        #if root is None:
        root = self.CQ
        
        #remove Vector objects
        objects_f = \
        {k:v for k,v in objects.items() if type(v.val()) not in (Vector,)}
        
        for name,shape in objects_f.items():
            ais = AIS_ColoredShape(shape.val().wrapped)
            ais.SetTransparency(alpha)
            ais_list.append(ais)
            root.addChild(ObjectTreeItem([name],
                                         shape=shape,
                                         ais=ais))
    
        self.sigObjectsAdded.emit(ais_list)
    
    @pyqtSlot(object,str,float)
    def addObject(self,object,name='',alpha=.0,):
        
        root = self.CQ
        
        ais = AIS_ColoredShape(object.val().wrapped)
        ais.SetTransparency(alpha)
        
        root.addChild(ObjectTreeItem([name],
                                     shape=object,
                                     ais=ais))
        
        self.sigObjectsAdded.emit([ais])
    
    
    @pyqtSlot(list)
    @pyqtSlot()
    def removeObjects(self,objects=None):
        
        if objects:
            removed_items_ais = [self.CQ.takeChild(i).ais for i in objects]
        else:
            removed_items_ais = [ch.ais for ch in self.CQ.takeChildren()]
            
        self.sigObjectsRemoved.emit(removed_items_ais)
    
    @pyqtSlot(bool)    
    def stashObjects(self,action : bool):
        
        if action:
            self._stash = self.CQ.takeChildren()
            removed_items_ais = [ch.ais for ch in self._stash]
            self.sigObjectsRemoved.emit(removed_items_ais)
        else:
            self.removeObjects()
            self.CQ.addChildren(self._stash)
            ais_list = [el.ais for el in self._stash]
            self.sigObjectsAdded.emit(ais_list)
        
    @pyqtSlot()    
    def removeSelected(self):
        
        ixs = self.selectedIndexes()
        rows = [ix.row() for ix in ixs]
        
        self.removeObjects(rows)
        
    @pyqtSlot()
    def exportSTL(self):
        
        item = self.selectedItems()[-1]
        if item.parent() is self.CQ:
            shape = item.shape
        else:
            return
        
        fname,_ = QFileDialog.getSaveFileName(self,filter='*stl')
        if fname is not '':
             with open(fname,'w') as f:
                exporters.exportShape(shape,
                                      exporters.ExportTypes.STL,
                                      f, 0.1)
    
    @pyqtSlot()    
    def handleSelection(self):
        
        item = self.selectedItems()[-1]
        if item.parent() is self.CQ:
            self.sigCQObjectSelected.emit(item.shape)
        