"""CadQuery execution backend of CQ-editor.

The GUI starts ``python -m cq_worker`` and exchanges JSON messages with it
(see :mod:`cq_worker.protocol`). Keeping CadQuery in its own process means a
crashing or endless script never takes the editor down with it.
"""

__version__ = "0.9.0"
