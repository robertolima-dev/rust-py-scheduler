"""
Django integration. We don't need a full Django project — a minimal
``settings.configure()`` is enough to import django.* without errors. The
behavior under test is in our own helper: start-once semantics and the
best-effort atexit registration.
"""
import time

import pytest

django = pytest.importorskip("django")


@pytest.fixture(autouse=True)
def _configure_django():
    from django.conf import settings

    if not settings.configured:
        settings.configure(
            DEBUG=True,
            INSTALLED_APPS=[],
            DATABASES={},
        )
    yield


from rust_py_scheduler import Scheduler  # noqa: E402
from rust_py_scheduler.django import start_in_background  # noqa: E402


def test_start_in_background_starts_the_scheduler():
    scheduler = Scheduler()
    calls = []
    scheduler.every("1s", lambda: calls.append(time.monotonic()))

    started = start_in_background(scheduler, register_atexit=False)
    try:
        assert started is True
        time.sleep(2.5)
        assert len(calls) >= 2
    finally:
        scheduler.shutdown()


def test_start_in_background_is_idempotent_per_process():
    scheduler = Scheduler()
    scheduler.every("1h", lambda: None)

    try:
        assert start_in_background(scheduler, register_atexit=False) is True
        # A second call must not raise RuntimeError ("already running") and
        # must report that it did nothing.
        assert start_in_background(scheduler, register_atexit=False) is False
    finally:
        scheduler.shutdown()


def test_start_in_background_registers_atexit_when_asked(monkeypatch):
    registered = []
    monkeypatch.setattr(
        "rust_py_scheduler.django.atexit.register",
        lambda fn: registered.append(fn),
    )

    scheduler = Scheduler()
    scheduler.every("1h", lambda: None)

    try:
        start_in_background(scheduler, register_atexit=True)
        assert scheduler.shutdown in registered
    finally:
        scheduler.shutdown()


def test_start_in_background_skips_atexit_when_disabled(monkeypatch):
    registered = []
    monkeypatch.setattr(
        "rust_py_scheduler.django.atexit.register",
        lambda fn: registered.append(fn),
    )

    scheduler = Scheduler()
    scheduler.every("1h", lambda: None)

    try:
        start_in_background(scheduler, register_atexit=False)
        assert registered == []
    finally:
        scheduler.shutdown()
