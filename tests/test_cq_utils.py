import cadquery as cq
import pytest

from cq_editor.cq_utils import to_compound


def test_to_compound_applies_sketch_placement():
    sk = cq.Sketch().rect(1, 1)
    placed = cq.Workplane("XZ").placeSketch(sk).val()
    f = to_compound(placed).face()

    assert f.Area() == pytest.approx(1)
    assert f.normalAt().toTuple() == pytest.approx((0, -1, 0))


def test_to_compound_empty_list():
    # #600: to_compound([]) used to raise IndexError while type-checking obj[0].
    result = to_compound([])

    assert isinstance(result, cq.Compound)
    assert result.Solids() == []
    assert result.Faces() == []
