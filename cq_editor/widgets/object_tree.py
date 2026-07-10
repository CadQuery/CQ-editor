from cadquery import Location, Assembly
from PyQt5.QtWidgets import (
    QTreeWidget,
    QTreeWidgetItem,
    QAction,
    QMenu,
    QWidget,
    QAbstractItemView,
    QButtonGroup,
    QRadioButton,
    QHeaderView,
    QHBoxLayout,
    QStyle,
)
from PyQt5.QtCore import Qt, pyqtSlot, pyqtSignal, QRect
from PyQt5 import sip

from pyqtgraph.parametertree import Parameter, ParameterTree

import qtawesome as qta

from OCP.AIS import AIS_Line
from OCP.Geom import Geom_CartesianPoint
from OCP.gp import gp_Pnt
from OCP.Bnd import Bnd_Box

from ..mixins import ComponentMixin
from ..icons import icon
from ..display import (
    DisplayMode,
    GlobalMode,
    effective_mode,
    HIDDEN_COL,
    WIREFRAME_COL,
    TRANSPARENT_COL,
    SHADED_COL,
    NAME_COL,
)
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

OBJECT_MODES = [
    DisplayMode.HIDDEN,
    DisplayMode.WIREFRAME,
    DisplayMode.TRANSPARENT,
    DisplayMode.SHADED,
]

GLOBAL_MODES = [
    GlobalMode.AS_SET,
    GlobalMode.WIREFRAME,
    GlobalMode.TRANSPARENT,
    GlobalMode.SHADED,
]

MODE_COLUMN_ICONS = (
    "fa5s.eye-slash",
    "mdi.vector-square",
    "fa5s.adjust",
    "fa5s.square",
)

MODE_COLUMN_WIDTH = 26


class CenteredIconHeader(QHeaderView):
    """
    A header that centres a column's icon.

    QHeaderView::paintSection only ever sets AlignVCenter on the section icon,
    so its horizontal alignment falls back to AlignLeft. setTextAlignment does
    not help - it aligns the label text, and the mode columns have none. The
    icons are therefore held here rather than on the header item, and painted
    centred over whatever the style drew.
    """

    def __init__(self, orientation, parent=None):

        super(CenteredIconHeader, self).__init__(orientation, parent)
        self._icons = {}

    def set_column_icon(self, col, icon):

        self._icons[col] = icon

    def paintSection(self, painter, rect, logicalIndex):

        painter.save()
        super(CenteredIconHeader, self).paintSection(painter, rect, logicalIndex)
        painter.restore()

        icon = self._icons.get(logicalIndex)
        if icon is None:
            return

        size = self.style().pixelMetric(QStyle.PM_SmallIconSize, None, self)
        target = QRect(0, 0, size, size)
        target.moveCenter(rect.center())
        icon.paint(painter, target)


class ModeRadioMixin(object):
    """
    A row of mutually exclusive radios, one per mode column. Auto-exclusivity
    cannot be used: setItemWidget reparents each radio into a different
    per-column widget, so they are not siblings. A QButtonGroup joins them.

    The class attributes stand in for __init__: QTreeWidgetItem's constructor
    is C++ and does not cascade into a Python mixin's __init__.
    """

    mode_group = None
    modes = ()
    mode_widgets = ()

    def build_mode_radios(self, tree, modes, tooltips):

        self.mode_group = QButtonGroup(tree)
        self.mode_widgets = []

        for col, tooltip in enumerate(tooltips):
            radio = QRadioButton()
            radio.setToolTip(tooltip)
            self.mode_group.addButton(radio, col)

            # setItemWidget stretches its widget across the whole cell, which
            # would pin each radio's indicator to the cell's left edge. Center
            # the radio inside a container so the indicators line up with the
            # centred header icons above them.
            container = QWidget()
            container.setToolTip(tooltip)
            box = QHBoxLayout(container)
            box.setContentsMargins(0, 0, 0, 0)
            box.addWidget(radio, 0, Qt.AlignCenter)

            tree.setItemWidget(self, col, container)
            self.mode_widgets.append(container)

        self.modes = list(modes)

    def set_mode_checked(self, mode):

        if self.mode_group is None:
            return

        button = self.mode_group.button(self.modes.index(mode))
        self.mode_group.blockSignals(True)
        button.setChecked(True)
        self.mode_group.blockSignals(False)

    def detach_mode_radios(self, tree):
        """
        Destroy the per-column item widgets and the QButtonGroup that joins
        them. Without this, the QButtonGroup - C++-parented to the
        long-lived tree - and its idClicked connections keep the item (and
        everything it references) alive after it is taken off the tree.

        Call this while the item is still on the tree. removeItemWidget()
        resolves a QModelIndex for the item, so on an item that has already
        been taken it silently no-ops - and the view is then left to release
        the widgets itself, which the sip.delete() below would race.

        removeItemWidget() only unsets the widget, it does not delete it, so
        the containers and the group are destroyed explicitly. Each radio dies
        with its container.
        """

        if self.mode_group is None:
            return

        for col in range(len(self.modes)):
            tree.removeItemWidget(self, col)

        for container in self.mode_widgets:
            if not sip.isdeleted(container):
                sip.delete(container)
        self.mode_widgets = []

        if not sip.isdeleted(self.mode_group):
            sip.delete(self.mode_group)
        self.mode_group = None


class TopTreeItem(QTreeWidgetItem):

    def __init__(self, *args, **kwargs):

        super(TopTreeItem, self).__init__(*args, **kwargs)


class ObjectTreeItem(ModeRadioMixin, QTreeWidgetItem):

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

        super(ObjectTreeItem, self).__init__(["", "", "", "", name], **kwargs)

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

        self.setData(NAME_COL, 0, self.properties["Name"])

        # if changed_prop.name() == "Alpha":
        #     self.ais.SetTransparency(self.properties["Alpha"])

        # if changed_prop.name() == "Color":
        #     set_color(self.ais, to_occ_color(self.properties["Color"]))

        # self.ais.Redisplay()

        if changed_prop.name() == "Display mode":
            mode = self.display_mode
            self.set_mode_checked(mode)

            # An item stands for its whole subtree - an assembly row often has
            # no shape of its own, so alone it would have nothing to show - so
            # the mode cascades to its parts. This is where the radios and the
            # properties editor's dropdown meet: the dropdown writes the
            # Parameter itself and never goes through the display_mode setter.
            for i in range(self.childCount()):
                self.child(i).display_mode = mode

        if self.sig:
            self.sig.emit()


class CQRootItem(TopTreeItem):

    def __init__(self, *args, **kwargs):

        super(CQRootItem, self).__init__(["CQ models"], *args, **kwargs)


class HelpersRootItem(TopTreeItem):

    def __init__(self, *args, **kwargs):

        super(HelpersRootItem, self).__init__(["Helpers"], *args, **kwargs)


class GlobalModeItem(ModeRadioMixin, TopTreeItem):

    def __init__(self, *args, **kwargs):

        super(GlobalModeItem, self).__init__(["", "", "", "", "All"], *args, **kwargs)
        self.setFlags(Qt.ItemIsEnabled)  # not selectable, no hover highlight


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

        header = CenteredIconHeader(Qt.Horizontal, tree)
        tree.setHeader(header)

        tree.setColumnCount(5)
        tree.setHeaderItem(QTreeWidgetItem(["", "", "", "", "Name"]))
        tree.setHeaderHidden(False)
        tree.setItemsExpandable(True)
        tree.setRootIsDecorated(False)
        tree.setContextMenuPolicy(Qt.ActionsContextMenu)

        # Assembly parts nest, so the branch indent has to be drawn in the name
        # column. Left in column 0 it would inset that column's cell, shifting
        # its radio right and shrinking it while columns 1-3 keep the full
        # section width.
        tree.setTreePosition(NAME_COL)

        header_item = tree.headerItem()
        for col, (icon_name, mode) in enumerate(zip(MODE_COLUMN_ICONS, OBJECT_MODES)):
            header.set_column_icon(col, qta.icon(icon_name))
            header_item.setToolTip(col, mode.value)
            header.setSectionResizeMode(col, QHeaderView.Fixed)
            tree.setColumnWidth(col, MODE_COLUMN_WIDTH)
        header_item.setTextAlignment(NAME_COL, Qt.AlignLeft | Qt.AlignVCenter)
        header.setSectionResizeMode(NAME_COL, QHeaderView.Stretch)

        # forward itemChanged signal, but only for helpers - CQ objects are
        # driven by their radios, and their NAME_COL carries no check state
        tree.itemChanged.connect(self._forward_item_changed)
        # handle visibility changes from tree
        tree.itemChanged.connect(self.handleChecked)

        self.GlobalItem = GlobalModeItem()
        self.CQ = CQRootItem()
        self.Helpers = HelpersRootItem()

        root = tree.invisibleRootItem()
        root.addChild(self.GlobalItem)
        root.addChild(self.CQ)
        root.addChild(self.Helpers)

        self.CQ.setFirstColumnSpanned(True)
        self.Helpers.setFirstColumnSpanned(True)

        self.GlobalItem.build_mode_radios(
            tree, GLOBAL_MODES, [m.value for m in GLOBAL_MODES]
        )
        self.GlobalItem.set_mode_checked(GlobalMode.AS_SET)
        self.GlobalItem.mode_group.idClicked.connect(self._handleGlobalRadio)

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
            item.setFlags(item.flags() | Qt.ItemIsUserCheckable)
            item.setCheckState(NAME_COL, Qt.Checked)
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

    def _attach_mode_radios(self, top):
        """
        Give every item in the subtree its own row of radios. Assembly parts
        are items in their own right, so they get their own modes too.

        setItemWidget needs the item to be on the tree already, so this runs
        after the subtree has been parented.
        """

        for item in self._iter_subtree(top):
            item.build_mode_radios(
                self.tree, OBJECT_MODES, [m.value for m in OBJECT_MODES]
            )
            item.set_mode_checked(item.display_mode)
            item.mode_group.idClicked.connect(
                lambda col, item=item: self._handleObjectRadio(item, col)
            )

    def _detach_mode_radios(self, top):

        for item in self._iter_subtree(top):
            item.detach_mode_radios(self.tree)

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

        # Every route into the global mode lands here - the toolbar actions,
        # the "All" row's own radios, and _handleObjectRadio's snap back to
        # AS_SET - so this is the one place that can keep the row in sync.
        self.GlobalItem.set_mode_checked(mode)

        self.sigGlobalModeChanged.emit(mode)
        self._apply_modes()

    @pyqtSlot(int)
    def _handleGlobalRadio(self, col):

        self.setGlobalMode(GLOBAL_MODES[col])

    def _handleObjectRadio(self, item, col):
        """
        Clicking a per-object radio releases the global override, so the click
        takes visible effect immediately. Clearing the override first means
        _apply_modes runs off the display_mode change that follows.

        The change cascades to the item's parts - see ObjectTreeItem.
        """

        self.setGlobalMode(GlobalMode.AS_SET)
        item.display_mode = OBJECT_MODES[col]

    def _forward_item_changed(self, item, col):

        if item.parent() is self.Helpers:
            self.sigItemChanged.emit(item, col)

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
                self._attach_mode_radios(item)

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
            self._attach_mode_radios(item)
        self.sigObjectsAdded.emit(ais_list)
        self._apply_modes()

    @pyqtSlot(list)
    @pyqtSlot()
    def removeObjects(self, objects=None):

        tops = (
            [self.CQ.child(i) for i in objects]
            if objects
            else [self.CQ.child(i) for i in range(self.CQ.childCount())]
        )
        for item in tops:
            self._detach_mode_radios(item)

        taken = (
            [self.CQ.takeChild(i) for i in objects]
            if objects
            else self.CQ.takeChildren()
        )

        removed_items_ais = self._subtree_ais(taken)

        self.sigObjectsRemoved.emit(removed_items_ais)

    @pyqtSlot(bool)
    def stashObjects(self, action: bool):

        if action:
            for i in range(self.CQ.childCount()):
                self._detach_mode_radios(self.CQ.child(i))
            self._stash = self.CQ.takeChildren()
            removed_items_ais = self._subtree_ais(self._stash)
            self.sigObjectsRemoved.emit(removed_items_ais)
        else:
            self.removeObjects()
            self.CQ.addChildren(self._stash)
            for item in self._stash:
                self._attach_mode_radios(item)
            ais_list = self._subtree_ais(self._stash)
            self.sigObjectsAdded.emit(ais_list)
            self._apply_modes()

    @pyqtSlot()
    def removeSelected(self):
        tops = [it for it in self.tree.selectedItems() if it.parent() is self.CQ]
        removed_items_ais = self._subtree_ais(tops)
        for it in tops:
            self._detach_mode_radios(it)
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

        if item.parent() is not self.Helpers:
            return

        if item.checkState(NAME_COL):
            if item.display_mode is DisplayMode.HIDDEN:
                item.display_mode = DisplayMode.SHADED
        else:
            item.display_mode = DisplayMode.HIDDEN
