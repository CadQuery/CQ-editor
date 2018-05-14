from PyQt5.QtWidgets import QTreeWidget, QTreeWidgetItem
from PyQt5.QtCore import Qt



class TopTreeItem(QTreeWidgetItem):
    
    def __init__(self,*args,**kwargs):
        
        super(TopTreeItem,self).__init__(*args,**kwargs)
        
class ObjectTreeItem(QTreeWidgetItem):
    
    def __init__(self,*args,**kwargs):
        
        super(ObjectTreeItem,self).__init__(*args,**kwargs)
        self.setFlags( self.flags() | Qt.ItemIsUserCheckable)
        self.setCheckState(0,Qt.Checked)

class CQRootItem(TopTreeItem):
    
    def __init__(self,*args,**kwargs):
        
        super(CQRootItem,self).__init__(['CQ models'],*args,**kwargs)
        
        
class ImportRootItem(TopTreeItem):
    
    def __init__(self,*args,**kwargs):
        
        super(ImportRootItem,self).__init__(['Imports'],*args,**kwargs)
        
class HelpersRootItem(TopTreeItem):
    
    def __init__(self,*args,**kwargs):
        
        super(HelpersRootItem,self).__init__(['Helpers'],*args,**kwargs)


class ObjectTree(QTreeWidget):
    
    def  __init__(self,parent):
        
        super(ObjectTree,self).__init__(parent)   
        self.setHeaderHidden(True)
        
        self.CQ = CQRootItem()
        self.Imports = ImportRootItem()
        self.Helpers = HelpersRootItem()
        
        root = self.invisibleRootItem()
        root.addChild(self.CQ)
        root.addChild(self.Imports)
        root.addChild(self.Helpers)
        
        self.CQ.addChild(ObjectTreeItem(['Dummy']))
        