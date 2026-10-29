"""What is picked in the GUI, as code: selectors and live shapes.

The GUI picks entities by their 1-based OCCT topology id.
:class:`PickedSelection` resolves its selection to the live shapes that the
console's ``selected()`` returns. A script needs a *selector* instead, such
as ``.faces(">Z")`` or ``.edges("|Z and >Y")``: :func:`selector_candidates`
finds the simplest selector strings that select exactly the picked entities
of one object, and ranks them.

Candidates are searched cheaply: the center, direction and geometry type of
every entity are read once, and each simple selector (``>Z``, ``|X``,
``%PLANE``, ``>Z[-2]`` ...) is evaluated on them with the same rules as
CadQuery's own selector classes. Combinations are then set operations on
bit masks. Nothing is trusted on that basis alone: every candidate returned
has been applied by CadQuery itself to the object the script applies it to,
and selected exactly the picked entities. When no geometric selector does,
an index into CadQuery's entity order is offered, flagged as such: it may
select another entity once the model changes.
"""

from __future__ import annotations

import itertools
import math
from collections.abc import Callable, Iterable, Sequence
from dataclasses import dataclass, field
from fractions import Fraction
from typing import Any

import cadquery as cq
import numpy as np
from .measurement import MeasurementError, TopologyReference, object_shape
from .scene import Registry, TopologyIndex, to_compound

#: CadQuery's default selector tolerance (``BaseDirSelector``, ``_NthSelector``).
TOLERANCE = 1e-4
#: Verified candidates returned, best first (an index fallback may follow).
MAX_CANDIDATES = 3
#: Candidates tried with CadQuery; each attempt evaluates every entity.
MAX_VERIFIED_ATTEMPTS = 10
#: Deepest ``>Z[-n]`` / ``>>Z[-n]`` generated.
MAX_NTH = 16
#: Most terms joined with ``or`` for several picked entities.
MAX_OR_TERMS = 4
#: Supersets of the picked entities combined three at a time.
MAX_TRIPLE_ATOMS = 30
#: Inclined directions offered as ``(x,y,z)`` selectors.
MAX_VECTOR_DIRECTIONS = 12
#: Cost of the index fallback: after every geometric selector.
INDEX_COST = 99.0

_METHODS = {"face": "faces", "edge": "edges", "vertex": "vertices"}
_ENTITY_LISTS = {"face": "Faces", "edge": "Edges", "vertex": "Vertices"}
_AXES = {"X": (1.0, 0.0, 0.0), "Y": (0.0, 1.0, 0.0), "Z": (0.0, 0.0, 1.0)}
_DIAGONALS = {"XY": (1.0, 1.0, 0.0), "XZ": (1.0, 0.0, 1.0), "YZ": (0.0, 1.0, 1.0)}


class SelectorError(MeasurementError):
    """The picked entities cannot be described by a selector."""


@dataclass(frozen=True)
class PickedSelection:
    """What the GUI had selected when it sent a console command."""

    #: Picked faces, edges and vertices, in the order they were picked.
    entities: tuple[TopologyReference, ...] = ()
    #: Keys of the selected objects (outermost first, in tree order).
    objects: tuple[str, ...] = ()

    @classmethod
    def from_message(cls, message: Any) -> PickedSelection:
        if message is None:
            return cls()
        if not isinstance(message, dict):
            raise MeasurementError("The selection must be an object")
        entities = message.get("entities", [])
        objects = message.get("objects", [])
        if not isinstance(entities, list) or not isinstance(objects, list):
            raise MeasurementError("Selected entities and objects must be lists")
        if not all(isinstance(key, str) and key for key in objects):
            raise MeasurementError("Selected objects must be scene keys")
        return cls(
            tuple(TopologyReference.from_message(value) for value in entities),
            tuple(objects),
        )

    def resolve(self, registry: Registry) -> list[cq.Shape]:
        """The picked entities placed as shown, or, when none is picked,
        the shapes of the selected objects."""

        try:
            if self.entities:
                return [
                    registry.get(r.key).subshape(r.part, r.kind, r.identifier)
                    for r in self.entities
                ]
            return [object_shape(registry, key) for key in self.objects]
        except KeyError as error:
            raise LookupError(error.args[0]) from None
        except MeasurementError as error:
            raise LookupError(str(error)) from None
        except (IndexError, ValueError) as error:
            raise LookupError(
                f"{error}; the model changed since it was picked, pick it again"
            ) from None


@dataclass(frozen=True)
class Atom:
    """A simple selector and the entities it selects, as a bit mask."""

    expression: str
    mask: int
    cost: float
    description: str


@dataclass(frozen=True)
class Candidate:
    """A selector string, or a complete index expression."""

    selector: str
    cost: float
    description: str
    #: The text to append to the object; set for index expressions.
    expression: str | None = None

    @property
    def index_based(self) -> bool:
        return self.expression is not None

    def to_message(self, method: str) -> dict:
        return {
            "expression": self.expression or f'.{method}("{self.selector}")',
            "selector": None if self.index_based else self.selector,
            "description": self.description,
            "index_based": self.index_based,
        }


@dataclass
class Features:
    """What simple selectors look at, for every entity of one object."""

    entities: list[cq.Shape]
    centers: np.ndarray  # (n, 3)
    #: Normal of planar faces, tangent of straight edges; NaN otherwise.
    directions: np.ndarray  # (n, 3)
    types: list[str]
    index: dict[cq.Shape, int] = field(default_factory=dict)

    @classmethod
    def of(cls, entities: Sequence[cq.Shape]) -> Features:
        centers = np.empty((len(entities), 3))
        directions = np.full((len(entities), 3), np.nan)
        types = []
        for i, entity in enumerate(entities):
            centers[i] = entity.Center().toTuple()
            kind = entity.ShapeType()
            geometry = entity.geomType() if kind in ("Face", "Edge") else kind.upper()
            types.append(geometry)
            # The vectors BaseDirSelector tests.
            if isinstance(entity, cq.Face) and geometry == "PLANE":
                directions[i] = entity.normalAt(None).toTuple()
            elif isinstance(entity, cq.Edge) and geometry == "LINE":
                directions[i] = entity.tangentAt().toTuple()
        return cls(
            list(entities),
            centers,
            directions,
            types,
            {entity: i for i, entity in enumerate(entities)},
        )


@dataclass
class Receiver:
    """The object a selector is appended to in the script."""

    obj: Any
    kind: str  # face, edge or vertex

    @property
    def method(self) -> str:
        return _METHODS[self.kind]

    @property
    def is_workplane(self) -> bool:
        return isinstance(self.obj, cq.Workplane)

    def entities(self) -> list[cq.Shape]:
        """Everything ``obj.<method>()`` starts from, in CadQuery's order."""

        if self.is_workplane:
            return list(getattr(self.obj, self.method)().vals())
        return list(getattr(self.obj, _ENTITY_LISTS[self.kind])())

    def select(self, selector: str) -> list[cq.Shape]:
        """What ``obj.<method>(selector)`` selects, as the script runs it."""

        result = getattr(self.obj, self.method)(selector)
        if self.is_workplane:
            return list(result.vals())
        return list(getattr(result, _ENTITY_LISTS[self.kind])())

    def index_expression(self, indices: Sequence[int]) -> str | None:
        if self.is_workplane:
            item = str(indices[0]) if len(indices) == 1 else str(list(indices))
            return f".{self.method}()[{item}]"
        if len(indices) == 1:
            # Shape.faces() is one compound; Shape.Faces() is the list.
            return f".{_ENTITY_LISTS[self.kind]}()[{indices[0]}]"
        return None

    def select_indices(self, indices: Sequence[int]) -> list[cq.Shape]:
        """What :meth:`index_expression` selects."""

        if self.is_workplane:
            selected = getattr(self.obj, self.method)()
            item = indices[0] if len(indices) == 1 else list(indices)
            return list(selected[item].vals())
        return [getattr(self.obj, _ENTITY_LISTS[self.kind])()[indices[0]]]


def selector_candidates(registry: Registry, raw_selections: Any) -> dict[str, Any]:
    """Selectors for the picked entities of one object, best first."""

    if not isinstance(raw_selections, list) or not raw_selections:
        raise SelectorError("Select faces, edges or vertices to get a selector")
    references = [TopologyReference.from_message(value) for value in raw_selections]
    first = references[0]
    if any((r.key, r.part) != (first.key, first.part) for r in references):
        raise SelectorError(
            "A selector describes entities of one object: pick them on a single object"
        )
    if any(r.kind != first.kind for r in references):
        raise SelectorError(
            "A selector describes one kind of entity: pick only faces, edges or vertices"
        )

    try:
        entry = registry.get(first.key)
    except KeyError as error:
        raise SelectorError(error.args[0]) from None
    if not 0 <= first.part < len(entry.sources):
        raise SelectorError(f"Unknown part {first.part}")
    source = entry.sources[first.part]
    if not isinstance(source.obj, (cq.Workplane, cq.Shape)):
        raise SelectorError(
            "Selectors are generated for workplanes and shapes, "
            f"not for a {type(source.obj).__name__}"
        )

    receiver = Receiver(source.obj, first.kind)
    features = entry.selector_features.get((first.part, first.kind))
    if features is None:
        features = Features.of(receiver.entities())
        entry.selector_features[(first.part, first.kind)] = features

    # Topology ids number the subshapes of the object's own (local) shape,
    # whatever location an assembly gives it.
    local = TopologyIndex.build(to_compound(source.obj).wrapped)
    targets = set()
    for reference in references:
        try:
            subshape = cq.Shape.cast(local.get(reference.kind, reference.identifier))
        except (IndexError, ValueError) as error:
            raise SelectorError(str(error)) from None
        index = features.index.get(subshape)
        if index is None:
            raise SelectorError(
                f"{reference.kind.title()} {reference.identifier} is not among "
                f"the object's .{receiver.method}()"
            )
        targets.add(index)

    candidates = _verified(receiver, features, targets, search(features, targets))
    if len(candidates) < MAX_CANDIDATES:
        fallback = _index_candidate(receiver, features, targets)
        if fallback is not None:
            candidates.append(fallback)
    if not candidates:
        raise SelectorError("No selector selects exactly these entities")
    messages = [c.to_message(receiver.method) for c in candidates]
    if first.kind == "face" and len(references) == 1:
        attribute = next(
            (a for a in entry.face_attributes if a["id"] == first.identifier), None
        )
        if attribute is not None and attribute.get("name"):
            for candidate in messages:
                candidate["description"] = (
                    f"{attribute['name']} · {candidate['description']}"
                )
    return {"method": receiver.method, "candidates": messages}


# -- simple selectors ------------------------------------------------------------


def atoms(features: Features) -> list[Atom]:
    """Every useful simple selector with what it selects."""

    count = len(features.entities)
    found: list[Atom] = []
    if count == 0:
        return found
    seen: set[str] = set()

    def add(expression: str, mask: int, cost: float, description: str) -> None:
        if mask and expression not in seen:
            seen.add(expression)
            found.append(Atom(expression, mask, cost, description))

    directions = features.directions
    has_direction = ~np.isnan(directions[:, 0])

    for name, vector in _AXES.items():
        v = np.array(vector)
        add(f">{name}", _extreme(features, v, True), 1.0, f"largest {name}")
        add(f"<{name}", _extreme(features, v, False), 1.0, f"smallest {name}")
        if has_direction.any():
            parallel = _parallel(directions, v)
            add(f"|{name}", _bits(parallel), 1.1, f"parallel to {name}")
            add(
                f"#{name}",
                _bits(_perpendicular(directions, v)),
                1.15,
                f"perpendicular to {name}",
            )
            add(f"+{name}", _bits(_aligned(directions, v)), 1.2, f"facing +{name}")
            add(f"-{name}", _bits(_aligned(directions, -v)), 1.2, f"facing -{name}")
            # >Z[n] only ranks what is parallel to Z (DirectionNthSelector).
            for n, (largest, smallest) in _nth(features, v, np.flatnonzero(parallel)):
                among = f"among those parallel to {name}"
                cost = 2.0 + 0.05 * n
                add(
                    f">{name}[-{n}]",
                    largest,
                    cost,
                    f"{_ordinal(n)} largest {name} {among}",
                )
                add(
                    f"<{name}[-{n}]",
                    smallest,
                    cost,
                    f"{_ordinal(n)} smallest {name} {among}",
                )
        for n, (largest, smallest) in _nth(features, v, np.arange(count), start=2):
            cost = 2.1 + 0.05 * n
            add(f">>{name}[-{n}]", largest, cost, f"{_ordinal(n)} largest {name}")
            add(f"<<{name}[-{n}]", smallest, cost, f"{_ordinal(n)} smallest {name}")

    for name, vector in _DIAGONALS.items():
        v = np.array(vector)
        sum_text = f"{name[0]}+{name[1]}"
        add(f">{name}", _extreme(features, v, True), 2.3, f"largest {sum_text}")
        add(f"<{name}", _extreme(features, v, False), 2.3, f"smallest {sum_text}")

    for geometry in sorted(set(features.types) - {"VERTEX"}):
        flags = np.array([t == geometry for t in features.types])
        add(f"%{geometry}", _bits(flags), 1.3, geometry.lower())

    # Inclined planar faces and straight edges, e.g. a chamfer.
    for direction in _distinct_directions(directions[has_direction]):
        text = _vector_text(direction)
        v = np.array([float(c) for c in text.strip("()").split(",")])
        add(f"+{text}", _bits(_aligned(directions, v)), 1.7, f"facing {text}")
        add(f"|{text}", _bits(_parallel(directions, v)), 1.8, f"parallel to {text}")
        add(f">{text}", _extreme(features, v, True), 2.6, f"farthest along {text}")
        add(f"<{text}", _extreme(features, v, False), 2.6, f"farthest against {text}")
    return found


def _bits(flags: np.ndarray) -> int:
    mask = 0
    for index in np.flatnonzero(flags):
        mask |= 1 << int(index)
    return mask


def _clusters(keys: np.ndarray, indices: Iterable[int]) -> list[list[int]]:
    """``_NthSelector.cluster``: ascending keys; a cluster spans the
    tolerance from its first key."""

    order = sorted((int(i) for i in indices), key=lambda i: keys[i])
    clusters: list[list[int]] = []
    start = 0.0
    for i in order:
        if not clusters or abs(keys[i] - start) > TOLERANCE:
            clusters.append([])
            start = keys[i]
        clusters[-1].append(i)
    return clusters


def _extreme(features: Features, vector: np.ndarray, largest: bool) -> int:
    """``>v`` / ``<v`` (DirectionMinMaxSelector)."""

    keys = features.centers @ vector
    clusters = _clusters(keys, range(len(keys)))
    return sum(1 << i for i in (clusters[-1] if largest else clusters[0]))


def _nth(
    features: Features, vector: np.ndarray, indices: np.ndarray, start: int = 1
) -> Iterable[tuple[int, tuple[int, int]]]:
    """The n-th largest and smallest cluster of ``indices`` along
    ``vector``, which ``v[-n]`` selects with ``>`` and ``<``."""

    if len(indices) == 0:
        return
    keys = features.centers @ vector
    clusters = _clusters(keys, indices)
    for n in range(start, min(len(clusters), MAX_NTH) + 1):
        yield n, (
            sum(1 << i for i in clusters[-n]),
            sum(1 << i for i in clusters[n - 1]),
        )


def _cross_length(directions: np.ndarray, vector: np.ndarray) -> np.ndarray:
    with np.errstate(invalid="ignore"):
        return np.linalg.norm(np.cross(vector, directions), axis=1)


def _angles(directions: np.ndarray, vector: np.ndarray) -> np.ndarray:
    with np.errstate(invalid="ignore"):
        return np.arctan2(_cross_length(directions, vector), directions @ vector)


def _parallel(directions: np.ndarray, vector: np.ndarray) -> np.ndarray:
    """``|v``: NaN directions (curved faces and edges) never match."""

    with np.errstate(invalid="ignore"):
        return _cross_length(directions, vector) < TOLERANCE


def _aligned(directions: np.ndarray, vector: np.ndarray) -> np.ndarray:
    with np.errstate(invalid="ignore"):
        return _angles(directions, vector) < TOLERANCE


def _perpendicular(directions: np.ndarray, vector: np.ndarray) -> np.ndarray:
    with np.errstate(invalid="ignore"):
        return np.abs(_angles(directions, vector) - math.pi / 2) < TOLERANCE


def _distinct_directions(directions: np.ndarray) -> list[np.ndarray]:
    """Unit directions off the axes, without duplicates or opposites."""

    found: list[np.ndarray] = []
    for direction in directions:
        if np.count_nonzero(np.abs(direction) > 1e-9) < 2:
            continue
        if any(abs(abs(float(direction @ other)) - 1.0) < 1e-12 for other in found):
            continue
        found.append(direction)
        if len(found) == MAX_VECTOR_DIRECTIONS:
            break
    return found


def _vector_text(direction: np.ndarray) -> str:
    """``(x,y,z)`` for selector strings: small integers when the direction
    has a simple ratio, otherwise six decimals."""

    largest = float(np.max(np.abs(direction)))
    for limit in range(1, 11):
        ratios = [
            Fraction(float(c) / largest).limit_denominator(limit) for c in direction
        ]
        denominator = math.lcm(*(r.denominator for r in ratios))
        integers = np.array([float(r * denominator) for r in ratios])
        if float(integers @ direction) / float(np.linalg.norm(integers)) > 1.0 - 1e-14:
            return "(" + ",".join(str(int(c)) for c in integers) + ")"
    return "(" + ",".join(_decimal(float(c)) for c in direction) + ")"


def _decimal(value: float) -> str:
    text = f"{value:.6f}".rstrip("0")
    return text + "0" if text.endswith(".") else text


def _ordinal(n: int) -> str:
    return {1: "1st", 2: "2nd", 3: "3rd"}.get(n, f"{n}th")


# -- combinations -----------------------------------------------------------------


def search(features: Features, targets: set[int]) -> list[Candidate]:
    """Candidate selectors for the entities at ``targets``, cheapest first.
    They follow CadQuery's rules but are not verified yet."""

    target = sum(1 << i for i in targets)
    found: dict[str, Candidate] = {}

    def offer(selector: str, cost: float, description: str) -> None:
        if selector not in found or found[selector].cost > cost:
            found[selector] = Candidate(selector, cost, description)

    simple = atoms(features)
    for atom in simple:
        if atom.mask == target:
            offer(atom.expression, atom.cost, atom.description)

    # "and" narrows selectors that each include every picked entity.
    supersets = sorted(
        (a for a in simple if a.mask & target == target and a.mask != target),
        key=lambda a: a.cost,
    )
    conjunctions: list[Atom] = list(supersets)
    for first, second in itertools.combinations(supersets[:MAX_TRIPLE_ATOMS], 2):
        conjunctions.append(_conjunction(first, second))
    for parts in itertools.combinations(supersets[:MAX_TRIPLE_ATOMS], 3):
        if parts[0].mask & parts[1].mask & parts[2].mask == target:
            conjunction = _conjunction(*parts)
            offer(conjunction.expression, conjunction.cost, conjunction.description)
    for conjunction in conjunctions:
        if conjunction.mask == target:
            offer(conjunction.expression, conjunction.cost, conjunction.description)

    # "exc" removes what a superset should not include ("and" binds
    # tighter, so "A and B exc C" is "(A and B) exc C").
    excluded = sorted((a for a in simple if a.mask & target == 0), key=lambda a: a.cost)
    for base in conjunctions:
        if base.mask & target != target or base.mask == target:
            continue
        for removed in excluded:
            if base.mask & ~removed.mask == target:
                offer(
                    f"{base.expression} exc {removed.expression}",
                    base.cost + removed.cost + 0.5,
                    f"{base.description}, except {removed.description}",
                )
                break

    if len(targets) > 1:
        union = _union(simple, supersets, target)
        if union is not None:
            offer(union.expression, union.cost, union.description)

    return sorted(found.values(), key=lambda c: (c.cost, len(c.selector), c.selector))


def _conjunction(*parts: Atom) -> Atom:
    mask = parts[0].mask
    for part in parts[1:]:
        mask &= part.mask
    return Atom(
        " and ".join(part.expression for part in parts),
        mask,
        sum(part.cost for part in parts) + 0.2 * (len(parts) - 1),
        ", ".join(part.description for part in parts),
    )


def _union(simple: list[Atom], supersets: list[Atom], target: int) -> Atom | None:
    """``a or b or ...`` of terms that each select only picked entities,
    chosen greedily."""

    terms = [atom for atom in simple if atom.mask & ~target == 0]
    for first, second in itertools.combinations(
        supersets[:MAX_TRIPLE_ATOMS] + simple, 2
    ):
        mask = first.mask & second.mask
        if mask and mask & ~target == 0:
            pair = _conjunction(first, second)
            terms.append(
                Atom(f"({pair.expression})", mask, pair.cost, pair.description)
            )
    chosen: list[Atom] = []
    covered = 0
    while covered != target and len(chosen) < MAX_OR_TERMS:
        useful = [term for term in terms if term.mask & ~covered]
        if not useful:
            return None
        best = max(
            useful,
            key=lambda term: (
                bin(term.mask & ~covered).count("1") / term.cost,
                -term.cost,
            ),
        )
        chosen.append(best)
        covered |= best.mask
    if covered != target:
        return None
    return Atom(
        " or ".join(term.expression for term in chosen),
        target,
        sum(term.cost for term in chosen) + 0.3 * (len(chosen) - 1),
        " or ".join(term.description for term in chosen),
    )


# -- verification ---------------------------------------------------------------


def _verified(
    receiver: Receiver,
    features: Features,
    targets: set[int],
    candidates: list[Candidate],
) -> list[Candidate]:
    """The candidates CadQuery itself confirms, best first."""

    expected = {features.entities[i] for i in targets}
    confirmed: list[Candidate] = []
    for candidate in candidates[:MAX_VERIFIED_ATTEMPTS]:
        if _selects(lambda: receiver.select(candidate.selector), expected):
            confirmed.append(candidate)
            if len(confirmed) == MAX_CANDIDATES:
                break
    return confirmed


def _index_candidate(
    receiver: Receiver, features: Features, targets: set[int]
) -> Candidate | None:
    indices = sorted(targets)
    expression = receiver.index_expression(indices)
    if expression is None:
        return None
    expected = {features.entities[i] for i in targets}
    if not _selects(lambda: receiver.select_indices(indices), expected):
        return None
    return Candidate(
        selector="",
        cost=INDEX_COST,
        description="by position in CadQuery's order; may change with the model",
        expression=expression,
    )


def _selects(select: Callable[[], list[cq.Shape]], expected: set[cq.Shape]) -> bool:
    try:
        selected = select()
    except Exception:  # not valid for this object, e.g. an empty nth cluster
        return False
    return len(selected) == len(expected) and set(selected) == expected
