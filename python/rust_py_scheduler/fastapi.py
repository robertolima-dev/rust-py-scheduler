"""
FastAPI integration for :class:`rust_py_scheduler.Scheduler`.

The scheduler runs on its own OS thread (``start_background()``), so it has
to be started when the app boots and stopped when it shuts down. FastAPI's
modern way to hook into that lifecycle is the ``lifespan`` context manager
(the older ``@app.on_event("startup"/"shutdown")`` API is deprecated), so
that's what this helper builds.

Usage::

    from fastapi import FastAPI
    from rust_py_scheduler import Scheduler
    from rust_py_scheduler.fastapi import scheduler_lifespan

    scheduler = Scheduler()

    @scheduler.every("30s")
    def heartbeat():
        ...

    app = FastAPI(lifespan=scheduler_lifespan(scheduler))

If you already have your own lifespan logic, wrap it instead — see
:func:`scheduler_lifespan`'s ``app_lifespan`` argument.
"""
from __future__ import annotations

from contextlib import asynccontextmanager
from typing import TYPE_CHECKING, AsyncIterator, Callable, Optional

if TYPE_CHECKING:  # pragma: no cover - typing only
    from fastapi import FastAPI

    from . import Scheduler


def scheduler_lifespan(
    scheduler: "Scheduler",
    app_lifespan: Optional[Callable[["FastAPI"], object]] = None,
):
    """Build a FastAPI ``lifespan`` that starts/stops ``scheduler``.

    The scheduler's background thread is started before the app begins
    serving requests and is shut down (and joined) when the app stops.

    :param scheduler: the :class:`Scheduler` to manage.
    :param app_lifespan: an optional existing lifespan context manager to
        compose with. If given, it's entered *inside* the scheduler's
        lifetime — so your startup runs after the scheduler is up, and your
        shutdown runs before the scheduler is stopped.
    :returns: an async context manager suitable for ``FastAPI(lifespan=...)``.
    """

    @asynccontextmanager
    async def lifespan(app: "FastAPI") -> AsyncIterator[None]:
        scheduler.start_background()
        try:
            if app_lifespan is not None:
                async with app_lifespan(app):
                    yield
            else:
                yield
        finally:
            scheduler.shutdown()

    return lifespan
