import inspect


def show_object(obj, name=None, options={}):
    return __call("show_object", obj, name, options)


def debug(obj, name=None):
    return __call("debug", obj, name)


def rand_color():
    return __call("rand_color")


def log(message):
    return __call("log", message)


def __call(key, *args, **kwargs):
    f = __find(key)
    if f:
        f(*args, **kwargs)


def __find(key):
    while True:
        f = inspect.currentframe()
        if f is None:
            return None
        f = f.f_back
        if f is None:
            return None
        if f.__module__ == "__cq_main__":
            break

    return f.f_locals.get(key, None)
