import inspect
from typing import Any, Callable


def __find(key):
    """
    1. Iterates stacks to find `__cq_main__` module.
    2. Returns a injected variable by given key.
    """
    for info in inspect.stack():
        if info.frame.f_globals["__name__"] == "__cq_main__":
            return info.frame.f_globals.get(key, None)
    return None


def __bind(key) -> Any:
    f = __find(key)

    def wrapped(*args, **kwargs):
        if f:
            return f(*args, **kwargs)

    return wrapped


show_object: Callable = __bind("show_object")
debug: Callable = __bind("debug")
rand_color: Callable[[], Any] = __bind("rand_color")
log: Callable[[Any], None] = __bind("log")
