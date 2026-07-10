from cadquery import Location, Assembly
from PyQt5.QtWidgets import (
    QTreeWidget,
    QTreeWidgetItem,
    QAction,
    QMenu,
    QWidget,
    QAbstractItemView,
)
from PyQt5.QtCore import Qt, pyqtSlot, pyqtSignal

from pyqtgraph.parametertree import Parameter, ParameterTree

from OCP.AIS import AIS_Line
from OCP.Geom import Geom_CartesianPoint
from OCP.gp import gp_Pnt
from OCP.Bnd import Bnd_Box

from ..mixins import ComponentMixin
from ..icons import icon
from ..display import DisplayMode, GlobalMode, effective_mode
from ..cq_utils import (
    make_AIS,
    export,
    to_occ_color,
    is_obj_empty,
    get_occ_color,
    set_color,
    set_transparency,
    to_compound,
)
from .viewer import DEFAULT_FACE_COLOR
from ..utils import splitter, layout, get_save_filename

# Default size of the axis helper lines half-length
DEFAULT_AXIS_HALF_LEN = 100.0


class TopTreeItem(QTreeWidgetItem):

    def __init__(self, *args, **kwargs):

        super(TopTreeItem, self).__init__(*args, **kwargs)


class ObjectTreeItem(QTreeWidgetItem):

    props = [
        {"name": "Name", "type": "str", "value": "", "readonly": True},
        # {"name": "Color", "type": "color", "value": "#f4a824"},
        # {"name": "Alpha", "type": "float", "value": 0, "limits": (0, 1), "step": 1e-1},
        {
            "name": "Display mode",
            "type": "list",
            "value": DisplayMode.SHADED.value,
            "values": [m.value for m in DisplayMode],
        },
    ]

    def __init__(
        self,
        name,
        ais=None,
        shape=None,
        shape_display=None,
        sig=None,
        alpha=0.0,
        color="#f4a824",
        **kwargs,
    ):

        super(ObjectTreeItem, self).__init__([name], **kwargs)
        self.setFlags(self.flags() | Qt.ItemIsUserCheckable)
        self.setCheckState(0, Qt.Checked)

        self.ais = ais
        self.shape = shape
        self.shape_display = shape_display
        self.sig = sig
        self.base_transparency = ais.Transparency() if ais is not None else 0.0

        self.properties = Parameter.create(name="Properties", children=self.props)

        self.properties["Name"] = name
        # Alpha and Color from this panel fight with the options in show_object and so they are
        # disabled for now until a better solution is found
        # self.properties["Alpha"] = ais.Transparency()
        # self.properties["Color"] = (
        #     get_occ_color(ais)
        #     if ais and ais.HasColor()
        #     else get_occ_color(DEFAULT_FACE_COLOR)
        # )
        self.properties.sigTreeStateChanged.connect(self.propertiesChanged)

    @property
    def display_mode(self) -> DisplayMode:

        return DisplayMode(self.properties["Display mode"])

    @display_mode.setter
    def display_mode(self, mode: DisplayMode):

        self.properties["Display mode"] = mode.value

    def propertiesChanged(self, properties, changed):

        changed_prop = changed[0][0]

        self.setData(0, 0, self.properties["Name"])

        # if changed_prop.name() == "Alpha":
        #     self.ais.SetTransparency(self.properties["Alpha"])

        # if changed_prop.name() == "Color":
        #     set_color(self.ais, to_occ_color(self.properties["Color"]))

        # self.ais.Redisplay()

        hidden = self.display_mode is DisplayMode.HIDDEN
        self.setCheckState(0, Qt.Unchecked if hidden else Qt.Checked)

        if self.sig:
            self.sig.emit()


class CQRootItem(TopTreeItem):

    def __init__(self, *args, **kwargs):

        super(CQRootItem, self).__init__(["CQ models"], *args, **kwargs)


class HelpersRootItem(TopTreeItem):

    def __init__(self, *args, **kwargs):

        super(HelpersRootItem, self).__init__(["Helpers"], *args, **kwargs)


class ObjectTree(QWidget, ComponentMixin):

    name = "Object Tree"
    _stash = []

    preferences = Parameter.create(
        name="Preferences",
        children=[
            {"name": "Preserve properties on reload", "type": "bool", "value": False},
            {"name": "Clear all before each run", "type": "bool", "value": True},
            {"name": "Merge Assemblies", "type": "bool", "value": False},
            {"name": "STL precision", "type": "float", "value": 0.1},
            {
                "name": "Transparency level",
                "type": "float",
                "value": 0.7,
                "limits": (0, 1),
                "step": 0.05,
            },
        ],
    )

    sigObjectsAdded = pyqtSignal([list], [list, bool])
    sigObjectsRemoved = pyqtSignal(list)
    sigCQObjectSelected = pyqtSignal(object)
    sigAISObjectsSelected = pyqtSignal(list)
    sigItemChanged = pyqtSignal(QTreeWidgetItem, int)
    sigObjectPropertiesChanged = pyqtSignal()
    sigHelpersResized = pyqtSignal(list)
    sigDisplayModesChanged = pyqtSignal(list)
    sigGlobalModeChanged = pyqtSignal(object)

    def __init__(self, parent):

        super(ObjectTree, self).__init__(parent)

        self._global_mode = GlobalMode.AS_SET

        # ObjectTree never calls ComponentMixin.__init__ (super() resolves to
        # QWidget.__init__, which does not cascade), so this connection - which
        # ComponentMixin would normally make - has to be made by hand.
        self.preferences.sigTreeStateChanged.connect(self.updatePreferences)

        self.tree = tree = QTreeWidget(
            self, selectionMode=QAbstractItemView.ExtendedSelection
        )
        self.properties_editor = ParameterTree(self)

        tree.setHeaderHidden(True)
        tree.setItemsExpandable(True)
        tree.setRootIsDecorated(False)
        tree.setContextMenuPolicy(Qt.ActionsContextMenu)

        # forward itemChanged singal
        tree.itemChanged.connect(lambda item, col: self.sigItemChanged.emit(item, col))
        # handle visibility changes form tree
        tree.itemChanged.connect(self.handleChecked)

        self.CQ = CQRootItem()
        self.Helpers = HelpersRootItem()

        root = tree.invisibleRootItem()
        root.addChild(self.CQ)
        root.addChild(self.Helpers)

        tree.expandToDepth(1)

        self._export_STL_action = QAction(
            "Export as STL",
            self,
            enabled=False,
            triggered=lambda: self.export("stl", self.preferences["STL precision"]),
        )

        self._export_STEP_action = QAction(
            "Export as STEP", self, enabled=False, triggered=lambda: self.export("step")
        )

        self._clear_current_action = QAction(
            icon("delete"),
            "Clear current",
            self,
            enabled=False,
            triggered=self.removeSelected,
        )

        self._toolbar_actions = [
            QAction(
                icon("delete-many"), "Clear all", self, triggered=self.removeObjects
            ),
            self._clear_current_action,
        ]

        self.prepareMenu()

        tree.itemSelectionChanged.connect(self.handleSelection)
        tree.customContextMenuRequested.connect(self.showMenu)

        self.prepareLayout()

        self.sigObjectPropertiesChanged.connect(self._apply_modes)

    def _axis_points(self, direction, halfLen):
        """Calculates the points needed to draw the axis helper lines"""
        p1 = Geom_CartesianPoint(gp_Pnt(*(-halfLen * d for d in direction)))
        p2 = Geom_CartesianPoint(gp_Pnt(*(halfLen * d for d in direction)))

        return p1, p2

    def _rescale_helpers(self):
        """Called to resize the axis helper lines to the model's bounding box."""

        # Bounding box over all currently displayed CQ shapes
        bbox = Bnd_Box()
        for i in range(self.CQ.childCount()):
            ais = self.CQ.child(i).ais
            if ais is not None:
                bbox.Add(ais.BoundingBox())

        if bbox.IsVoid():
            halfLen = DEFAULT_AXIS_HALF_LEN
        else:
            halfLen = 2.0 * bbox.CornerMin().Distance(bbox.CornerMax())

        resized = []
        for item, direction in self._helper_dirs:
            p1, p2 = self._axis_points(direction, halfLen)
            item.ais.SetPoints(p1, p2)  # update geometry in place
            resized.append(item.ais)

        if resized:
            self.sigHelpersResized.emit(resized)

    def prepareMenu(self):

        self.tree.setContextMenuPolicy(Qt.CustomContextMenu)

        self._context_menu = QMenu(self)
        self._context_menu.addActions(self._toolbar_actions)
        self._context_menu.addActions(
            (self._export_STL_action, self._export_STEP_action)
        )

    def prepareLayout(self):

        self._splitter = splitter(
            (self.tree, self.properties_editor),
            stretch_factors=(2, 1),
            orientation=Qt.Vertical,
        )
        layout(self, (self._splitter,), top_widget=self)

        self._splitter.show()

    def showMenu(self, position):

        self._context_menu.exec_(self.tree.viewport().mapToGlobal(position))

    def menuActions(self):

        return {"Tools": [self._export_STL_action, self._export_STEP_action]}

    def toolbarActions(self):

        return self._toolbar_actions

    def addLines(self):
        ais_list = []
        self._helper_dirs = []  # (item, direction) pairs, for later rescaling

        for name, color, direction in zip(
            ("X", "Y", "Z"),
            ("red", "lawngreen", "blue"),
            ((1, 0, 0), (0, 1, 0), (0, 0, 1)),
        ):
            p1, p2 = self._axis_points(direction, DEFAULT_AXIS_HALF_LEN)
            line = AIS_Line(p1, p2)
            line.SetColor(to_occ_color(color))

            item = ObjectTreeItem(name, ais=line)
            self.Helpers.addChild(item)
            self._helper_dirs.append((item, direction))

            ais_list.append(line)

        self.sigObjectsAdded.emit(ais_list)

    def _item_path(self, item):
        """Stable identity of an item across runs: names from the CQ root down."""
        parts = []
        node = item
        while node is not None and node is not self.CQ:
            parts.append(node.properties["Name"])
            node = node.parent()
        return "/".join(reversed(parts))

    def _current_properties(self):
        """
        Snapshot every CQ item's properties before a reload, keyed by tree
        path so nested parts that share a name do not collide.
        """
        current_params = {}
        for i in range(self.CQ.childCount()):
            for it in self._iter_subtree(self.CQ.child(i)):
                current_params[self._item_path(it)] = it.properties

        return current_params

    def _restore_properties(self, obj, properties):
        """
        Re-apply a snapshot from _current_properties to obj and all its
        descendants, matching items by tree path.
        """
        for it in self._iter_subtree(obj):
            key = self._item_path(it)
            if key in properties:
                for p in properties[key]:
                    it.properties[p.name()] = p.value()

    def _build_assembly_item(
        self, node, label, options, parent_loc, inherited_color, ais_list
    ):
        """
        Recursively build the tree item for one assembly node, mirroring
        the hierarchy. Accumulates world location and nearest-ancestor color.
        """
        world = parent_loc * node.loc
        color = node.color if node.color is not None else inherited_color

        ais = None
        shape = None
        if node.obj is not None:
            # A node can have both a shape and children
            shape = to_compound(node.obj).moved(world)
            ais, _ = make_AIS(shape, options)
            if color is not None:
                r, g, b, a = color.toTuple()
                set_color(ais, to_occ_color((r, g, b)))
                set_transparency(ais, a)
            ais_list.append(ais)

        item = ObjectTreeItem(
            label, shape=shape, ais=ais, sig=self.sigObjectPropertiesChanged
        )

        for child in node.children:
            item.addChild(
                self._build_assembly_item(
                    child, child.name, options, world, color, ais_list
                )
            )

        if node.children:
            item.setFlags(item.flags() | Qt.ItemIsAutoTristate)

        return item

    def _build_items(self, name, shape, options):
        """
        Build the ObjectTreeItem(s) for one shown object. Assemblies explode
        into one item per part. Everything else is one item.
        """
        # Explode assemblies into per-part items
        if isinstance(shape, Assembly) and not self.preferences["Merge Assemblies"]:
            ais_list = []
            item = self._build_assembly_item(
                shape, name, options, Location(), None, ais_list
            )
            return [item], ais_list

        ais, shape_display = make_AIS(shape, options)
        item = ObjectTreeItem(
            name,
            shape=shape,
            shape_display=shape_display,
            ais=ais,
            sig=self.sigObjectPropertiesChanged,
        )
        return [item], [ais]

    def _iter_subtree(self, item):
        """Yield item and every descendant."""
        yield item
        for i in range(item.childCount()):
            yield from self._iter_subtree(item.child(i))

    def _under_cq(self, item):
        """True if item is anywhere beneath the CQ root (exluding Helpers)."""
        p = item.parent()
        while p is not None:
            if p is self.CQ:
                return True
            p = p.parent()
        return False

    def _subtree_ais(self, tops):
        return [
            it.ais
            for top in tops
            for it in self._iter_subtree(top)
            if it.ais is not None
        ]

    def _visible_ais(self, tops):
        """_subtree_ais, minus the items that start out hidden."""
        return [
            it.ais
            for top in tops
            for it in self._iter_subtree(top)
            if it.ais is not None and it.display_mode is not DisplayMode.HIDDEN
        ]

    @property
    def global_mode(self) -> GlobalMode:

        return self._global_mode

    @pyqtSlot(object)
    def setGlobalMode(self, mode: GlobalMode):

        if mode is self._global_mode:
            return

        self._global_mode = mode
        self.sigGlobalModeChanged.emit(mode)
        self._apply_modes()

    @pyqtSlot()
    def _apply_modes(self):
        """
        Resolve every CQ object's effective mode and transparency and hand the
        result to the viewer. Assembly parts are nested items and carry their
        own mode, so the whole subtree is walked. Helpers are excluded - they
        keep their checkbox and are not affected by the global override.
        """

        transparency = self.preferences["Transparency level"]

        payload = []
        for i in range(self.CQ.childCount()):
            for item in self._iter_subtree(self.CQ.child(i)):
                if item.ais is None:
                    continue
                mode = effective_mode(item.display_mode, self._global_mode)
                t = (
                    transparency
                    if mode is DisplayMode.TRANSPARENT
                    else item.base_transparency
                )
                payload.append((item.ais, mode, t))

        if payload:
            self.sigDisplayModesChanged.emit(payload)

    @pyqtSlot(object, object)
    def updatePreferences(self, *args):

        self._apply_modes()

    @pyqtSlot(dict, bool)
    @pyqtSlot(dict)
    def addObjects(self, objects, clean=False, root=None):

        if root is None:
            root = self.CQ

        request_fit_view = True if root.childCount() == 0 else False
        preserve_props = self.preferences["Preserve properties on reload"]

        if preserve_props:
            current_props = self._current_properties()

        if clean or self.preferences["Clear all before each run"]:
            self.removeObjects()

        ais_list = []

        # remove empty objects
        objects_f = {k: v for k, v in objects.items() if not is_obj_empty(v.shape)}

        for name, obj in objects_f.items():
            top_items, _ = self._build_items(name, obj.shape, obj.options)
            for item in top_items:
                if preserve_props and name in current_props:
                    self._restore_properties(item, current_props)
                self.CQ.addChild(item)
                self.tree.expandItem(item)

            ais_list.extend(self._visible_ais(top_items))

        if request_fit_view:
            self.sigObjectsAdded[list, bool].emit(ais_list, True)
        else:
            self.sigObjectsAdded[list].emit(ais_list)

        self._apply_modes()
        self._rescale_helpers()

    @pyqtSlot(object, str, object)
    def addObject(self, obj, name="", options=None):

        if options is None:
            options = {}

        root = self.CQ

        top_items, ais_list = self._build_items(name, obj, options)
        for item in top_items:
            self.CQ.addChild(item)
        self.sigObjectsAdded.emit(ais_list)
        self._apply_modes()

    @pyqtSlot(list)
    @pyqtSlot()
    def removeObjects(self, objects=None):

        taken = (
            [self.CQ.takeChild(i) for i in objects]
            if objects
            else self.CQ.takeChildren()
        )
        removed_items_ais = [
            it.ais
            for top in taken
            for it in self._iter_subtree(top)
            if it.ais is not None
        ]

        self.sigObjectsRemoved.emit(removed_items_ais)

    @pyqtSlot(bool)
    def stashObjects(self, action: bool):

        if action:
            self._stash = self.CQ.takeChildren()
            # removed_items_ais = [ch.ais for ch in self._stash]
            removed_items_ais = self._subtree_ais(self._stash)
            self.sigObjectsRemoved.emit(removed_items_ais)
        else:
            self.removeObjects()
            self.CQ.addChildren(self._stash)
            ais_list = self._subtree_ais(self._stash)
            self.sigObjectsAdded.emit(ais_list)
            self._apply_modes()

    @pyqtSlot()
    def removeSelected(self):
        tops = [it for it in self.tree.selectedItems() if it.parent() is self.CQ]
        removed_items_ais = self._subtree_ais(tops)
        for it in tops:
            self.CQ.removeChild(it)
        self.sigObjectsRemoved.emit(removed_items_ais)

    def export(self, export_type, precision=None):

        items = self.tree.selectedItems()

        # If the CQ root is selected, take all top-level objects
        if any(it is self.CQ for it in items):
            roots = [self.CQ.child(i) for i in range(self.CQ.childCount())]
        # Otherwise take every selected item anywhere under CQ
        else:
            roots = [it for it in items if self._under_cq(it)]

        seen = set()
        shapes = []
        for r in roots:
            for it in self._iter_subtree(r):
                if it.shape is not None and id(it) not in seen:
                    seen.add(id(it))
                    shapes.append(it.shape)

        fname = get_save_filename(export_type)
        if fname != "":
            export(shapes, export_type, fname, precision)

    @pyqtSlot()
    def handleSelection(self):

        items = self.tree.selectedItems()
        if len(items) == 0:
            self._export_STL_action.setEnabled(False)
            self._export_STEP_action.setEnabled(False)
            return

        # emit list of all selected ais objects (might be empty)
        # ais_objects = [item.ais for item in items if item.parent() is self.CQ]
        ais_objects = [
            it.ais
            for sel in items
            if self._under_cq(sel)
            for it in self._iter_subtree(sel)
            if it.ais is not None
        ]
        self.sigAISObjectsSelected.emit(ais_objects)

        # Context menu + last-selected object
        item = items[-1]
        if self._under_cq(item):
            self._export_STL_action.setEnabled(True)
            self._export_STEP_action.setEnabled(True)
            self._clear_current_action.setEnabled(True)
            if item.shape is not None:
                self.sigCQObjectSelected.emit(item.shape)
            self.properties_editor.setParameters(item.properties, showTop=False)
            self.properties_editor.setEnabled(True)
        elif item is self.CQ and item.childCount() > 0:
            self._export_STL_action.setEnabled(True)
            self._export_STEP_action.setEnabled(True)
        else:
            self._export_STL_action.setEnabled(False)
            self._export_STEP_action.setEnabled(False)
            self._clear_current_action.setEnabled(False)
            self.properties_editor.setEnabled(False)
            self.properties_editor.clear()

    @pyqtSlot(list)
    def handleGraphicalSelection(self, shapes):

        self.tree.clearSelection()

        for item in self._iter_subtree(self.CQ):
            ais = getattr(item, "ais", None)
            if ais is None:
                continue
            if any(ais.Shape().IsEqual(shape) for shape in shapes):
                item.setSelected(True)

    @pyqtSlot(QTreeWidgetItem, int)
    def handleChecked(self, item, col):

        if type(item) is not ObjectTreeItem:
            return

        if item.checkState(0):
            if item.display_mode is DisplayMode.HIDDEN:
                item.display_mode = DisplayMode.SHADED
        else:
            item.display_mode = DisplayMode.HIDDEN
