"""
FastAPI integration: the scheduler should start with the app and stop when
the app's lifespan exits. We drive the full lifespan via TestClient used as a
context manager, which is what actually triggers startup/shutdown.
"""
import time

import pytest

fastapi = pytest.importorskip("fastapi")
from fastapi import FastAPI  # noqa: E402
from fastapi.testclient import TestClient  # noqa: E402

from rust_py_scheduler import Scheduler  # noqa: E402
from rust_py_scheduler.fastapi import scheduler_lifespan  # noqa: E402


def test_scheduler_runs_while_the_app_is_up():
    scheduler = Scheduler()
    calls = []
    scheduler.every("1s", lambda: calls.append(time.monotonic()))

    app = FastAPI(lifespan=scheduler_lifespan(scheduler))

    @app.get("/")
    def index():
        return {"ok": True}

    with TestClient(app) as client:
        assert client.get("/").json() == {"ok": True}
        time.sleep(2.5)
        assert len(calls) >= 2

    # After the context manager exits, lifespan shutdown ran -> the scheduler
    # was stopped and joined. Calling shutdown() again must be a safe no-op.
    scheduler.shutdown()


def test_scheduler_is_stopped_after_lifespan_exit():
    scheduler = Scheduler()
    calls = []
    scheduler.every("1s", lambda: calls.append(time.monotonic()))

    app = FastAPI(lifespan=scheduler_lifespan(scheduler))

    with TestClient(app):
        time.sleep(1.5)

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

    with TestClient(app):
        assert events == ["startup"]

    assert events == ["startup", "shutdown"]
