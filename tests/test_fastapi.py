"""
FastAPI integration: the scheduler should start with the app and stop when
the app's lifespan exits.

We drive the ``lifespan`` context manager directly with asyncio rather than
going through ``fastapi.testclient.TestClient``. That's both a more focused
test (it exercises *our* lifespan, not Starlette's HTTP plumbing) and avoids
TestClient's httpx/httpx2 dependency churn across Starlette versions.
"""
import asyncio
import time

import pytest

fastapi = pytest.importorskip("fastapi")
from fastapi import FastAPI  # noqa: E402

from rust_py_scheduler import Scheduler  # noqa: E402
from rust_py_scheduler.fastapi import scheduler_lifespan  # noqa: E402


def test_scheduler_runs_while_the_app_is_up():
    scheduler = Scheduler()
    calls = []
    scheduler.every("1s", lambda: calls.append(time.monotonic()))

    app = FastAPI(lifespan=scheduler_lifespan(scheduler))

    async def drive():
        async with app.router.lifespan_context(app):
            # The scheduler's background thread is running here.
            time.sleep(2.5)
            assert len(calls) >= 2

    asyncio.run(drive())

    # After the lifespan exits, shutdown ran -> calling it again is a no-op.
    scheduler.shutdown()


def test_scheduler_is_stopped_after_lifespan_exit():
    scheduler = Scheduler()
    calls = []
    scheduler.every("1s", lambda: calls.append(time.monotonic()))

    app = FastAPI(lifespan=scheduler_lifespan(scheduler))

    async def drive():
        async with app.router.lifespan_context(app):
            time.sleep(1.5)

    asyncio.run(drive())

    calls_after_shutdown = len(calls)
    assert calls_after_shutdown >= 1

    # No further executions should happen once the app (and scheduler) stopped.
    time.sleep(1.5)
    assert len(calls) == calls_after_shutdown


def test_composes_with_an_existing_lifespan():
    from contextlib import asynccontextmanager

    scheduler = Scheduler()
    events = []

    @asynccontextmanager
    async def my_lifespan(app):
        events.append("startup")
        yield
        events.append("shutdown")

    app = FastAPI(lifespan=scheduler_lifespan(scheduler, my_lifespan))

    async def drive():
        async with app.router.lifespan_context(app):
            assert events == ["startup"]

    asyncio.run(drive())

    assert events == ["startup", "shutdown"]
