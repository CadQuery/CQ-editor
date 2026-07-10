from enum import Enum


class DisplayMode(Enum):
    """How a single object is drawn. HIDDEN means it is erased from the view."""

    HIDDEN = "Hidden"
    WIREFRAME = "Wireframe"
    TRANSPARENT = "Transparent"
    SHADED = "Shaded"


class GlobalMode(Enum):
    """Panel-wide override. AS_SET means every object keeps its own DisplayMode."""

    AS_SET = "As set below"
    WIREFRAME = "Wireframe"
    TRANSPARENT = "Transparent"
    SHADED = "Shaded"


def effective_mode(item: DisplayMode, glob: GlobalMode) -> DisplayMode:

    # An override never unhides an object.
    if item is DisplayMode.HIDDEN:
        return DisplayMode.HIDDEN

    if glob is GlobalMode.AS_SET:
        return item

    return DisplayMode[glob.name]
