"""
Formal contract for every exception `rust_py_scheduler` can raise: which
Python exception type, in which situation. Other test files exercise the
surrounding *behavior* (e.g. that the run loop keeps going after a job
fails); this file exists so the exception types themselves are pinned down
in one place.
"""
import threading
import time

import pytest

from rust_py_scheduler import Scheduler


def test_invalid_interval_raises_value_error():
    scheduler = Scheduler()

    with pytest.raises(ValueError):
        scheduler.every("not-a-valid-interval", lambda: None)


def test_invalid_interval_raises_value_error_for_decorator_form():
    scheduler = Scheduler()

    with pytest.raises(ValueError):
        @scheduler.every("not-a-valid-interval")
        def decorated():
            pass


def test_starting_background_twice_raises_runtime_error():
    scheduler = Scheduler()
    scheduler.every("1h", lambda: None)
    scheduler.start_background()

    try:
        with pytest.raises(RuntimeError):
            scheduler.start_background()
    finally:
        scheduler.shutdown()


def test_removing_an_unknown_job_id_raises_key_error():
    scheduler = Scheduler()

    with pytest.raises(KeyError):
        scheduler.remove_job("not-a-real-id")


def test_callback_exceptions_are_caught_and_never_propagate_out_of_run():
    scheduler = Scheduler()

    def boom():
        raise RuntimeError("falha proposital")

    scheduler.every("1s", boom)

    thread = threading.Thread(target=scheduler.run)
    thread.start()
    time.sleep(1.4)
    scheduler.shutdown()
    thread.join(timeout=2)

    assert not thread.is_alive()
    assert scheduler.list_jobs()[0]["error_count"] >= 1


def test_a_non_callable_registered_as_a_job_fails_gracefully_at_run_time():
    # `every()` doesn't validate that `callback` is actually callable -- only
    # calling it does. This documents that such a mistake degrades to a
    # caught-and-counted error instead of crashing the scheduling loop.
    scheduler = Scheduler()
    scheduler.every("1s", "not-a-callable")

    thread = threading.Thread(target=scheduler.run)
    thread.start()
    time.sleep(1.4)
    scheduler.shutdown()
    thread.join(timeout=2)

    assert not thread.is_alive()
    job = scheduler.list_jobs()[0]
    assert job["error_count"] >= 1
    assert "not callable" in job["last_error"]
