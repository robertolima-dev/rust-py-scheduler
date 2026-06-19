"""
Django integration for :class:`rust_py_scheduler.Scheduler`.

Django has a clean *startup* hook (``AppConfig.ready()``) but, unlike
FastAPI's ``lifespan``, no guaranteed *shutdown* hook — a process can be
killed by a signal, and under gunicorn/uwsgi each worker is its own process.
So this helper does two honest things:

1. ``start_in_background()`` — call it from ``AppConfig.ready()`` to start the
   scheduler. It's idempotent per process: calling it twice won't spawn a
   second background thread or raise.
2. a best-effort ``atexit`` handler that calls ``shutdown()`` on normal
   interpreter exit. This does *not* cover ``kill -9`` — for graceful
   shutdown under a signal, install your own ``SIGTERM`` handler.

Important caveat for multi-worker deployments (gunicorn/uwsgi with
``workers > 1``): every worker process runs ``ready()`` and therefore its own
scheduler, so your jobs run once *per worker*. There is no cross-process
coordination. If you need a job to run exactly once cluster-wide, run the
scheduler in a single dedicated process (e.g. a management command) instead
of in the web workers.

Usage (in your app's ``apps.py``)::

    from django.apps import AppConfig
    from rust_py_scheduler import Scheduler
    from rust_py_scheduler.django import start_in_background

    scheduler = Scheduler()

    @scheduler.every("5m")
    def refresh_cache():
        ...

    class MyAppConfig(AppConfig):
        name = "myapp"

        def ready(self):
            start_in_background(scheduler)
"""
from __future__ import annotations

import atexit
import threading
from typing import TYPE_CHECKING

if TYPE_CHECKING:  # pragma: no cover - typing only
    from . import Scheduler

# Guards against AppConfig.ready() running more than once in the same process
# (Django can import app configs more than once in some setups), which would
# otherwise make start_background() raise "already running". We hold the
# scheduler objects themselves (not their id()), both because id() is reused
# after an object is garbage-collected and because an app-level scheduler is
# meant to live for the whole process anyway.
_started_lock = threading.Lock()
_started: set = set()


def start_in_background(scheduler: "Scheduler", register_atexit: bool = True) -> bool:
    """Start ``scheduler`` on its background thread, once per process.

    Safe to call from ``AppConfig.ready()``. If this exact scheduler was
    already started in this process, it's a no-op and returns ``False``.

    :param scheduler: the :class:`Scheduler` to start.
    :param register_atexit: when true (default), register a best-effort
        ``atexit`` handler that calls ``scheduler.shutdown()`` on normal
        interpreter exit. Does not cover ``kill -9``.
    :returns: ``True`` if this call started the scheduler, ``False`` if it was
        already running in this process.
    """
    with _started_lock:
        if scheduler in _started:
            return False
        scheduler.start_background()
        _started.add(scheduler)

    if register_atexit:
        atexit.register(scheduler.shutdown)

    return True
